#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "std"), no_main)]

#[ink::contract]
mod token_swap {
    use ink::storage::Mapping;
    use ink::LangError;
    use ink_env::call::{build_call, ExecutionInput, Selector};
    use ink_env::DefaultEnvironment;

    pub type Swap = (
        AccountId,   // creator
        AccountId,   // token_a
        AccountId,   // token_b
        Balance,     // amount_a
        Balance,     // amount_b
        BlockNumber, // expiration
    );

    #[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        SwapNotFound,
        InsufficientBalance,
        Unauthorized,
        SwapExpired,
        TransferFailed,
        CallFailed,
    }

    pub type Result<T> = core::result::Result<T, Error>;

    #[ink(storage)]
    pub struct TokenSwap {
        pub swaps: Mapping<u64, Swap>,
        pub swap_count: u64,
    }

    #[ink(event)]
    pub struct SwapCreated {
        #[ink(topic)]
        id: u64,
        #[ink(topic)]
        creator: AccountId,
    }

    #[ink(event)]
    pub struct SwapAccepted {
        #[ink(topic)]
        id: u64,
        #[ink(topic)]
        acceptor: AccountId,
    }

    #[ink(event)]
    pub struct SwapDeleted {
        #[ink(topic)]
        id: u64,
    }

    impl TokenSwap {
        #[ink(constructor)]
        pub fn new() -> Self {
            Self {
                swaps: Default::default(),
                swap_count: 0,
            }
        }

        fn get_balance(&self, token_contract: AccountId, account: AccountId) -> Result<Balance> {
            let result: core::result::Result<
                core::result::Result<Balance, LangError>,
                ink_env::Error,
            > = build_call::<DefaultEnvironment>()
                .call(token_contract)
                .gas_limit(5000)
                .transferred_value(0)
                .exec_input(
                    ExecutionInput::new(Selector::new(ink::selector_bytes!("balance_of")))
                        .push_arg(account),
                )
                .returns::<Balance>()
                .try_invoke();

            match result {
                Ok(Ok(balance)) => Ok(balance),
                Ok(Err(_)) => Err(Error::InsufficientBalance),
                Err(_) => Err(Error::CallFailed),
            }
        }

        pub fn create_swap(
            &mut self,
            token_a: AccountId,
            token_b: AccountId,
            amount_a: Balance,
            amount_b: Balance,
            duration: BlockNumber,
        ) -> Result<u64> {
            let caller = self.env().caller();
            let balance_a: Balance = self.get_balance(token_a, caller)?;

            if balance_a < amount_a {
                return Err(Error::InsufficientBalance);
            }

            self.transfer_token(token_a, caller, self.env().account_id(), amount_a)?;

            let expiration = self
                .env()
                .block_number()
                .checked_add(duration)
                .ok_or(Error::CallFailed)?;

            let new_swap = (caller, token_a, token_b, amount_a, amount_b, expiration);

            self.swaps.insert(self.swap_count, &new_swap);
            let id = self.swap_count;
            self.swap_count = self.swap_count.checked_add(1).ok_or(Error::CallFailed)?;

            self.env().emit_event(SwapCreated {
                id,
                creator: caller,
            });

            Ok(id)
        }

        #[ink(message)]
        pub fn delete_swap(&mut self, swap_id: u64) -> Result<()> {
            let caller = self.env().caller();
            let swap = self.swaps.get(&swap_id).ok_or(Error::SwapNotFound)?;

            if caller != swap.0 {
                return Err(Error::Unauthorized);
            }

            self.transfer_token(swap.1, self.env().account_id(), swap.0, swap.3)?;
            self.swaps.take(&swap_id);

            self.env().emit_event(SwapDeleted { id: swap_id });

            Ok(())
        }

        #[ink(message)]
        pub fn accept_swap(&mut self, swap_id: u64) -> Result<()> {
            let swap = self.swaps.take(&swap_id).ok_or(Error::SwapNotFound)?;

            let caller = self.env().caller();

            let balance_b: Balance = self.get_balance(swap.2, caller)?;
            if balance_b < swap.4 {
                return Err(Error::InsufficientBalance);
            }

            if self.env().block_number() > swap.5 {
                return Err(Error::SwapExpired);
            }

            self.transfer_token(swap.1, self.env().account_id(), caller, swap.3)?;
            self.transfer_token(swap.2, caller, swap.0, swap.4)?;

            self.env().emit_event(SwapAccepted {
                id: swap_id,
                acceptor: caller,
            });

            Ok(())
        }

        fn transfer_token(
            &self,
            token_contract: AccountId,
            from: AccountId,
            to: AccountId,
            amount: Balance,
        ) -> Result<()> {
            let result = ink_env::call::build_call::<DefaultEnvironment>()
                .call(token_contract)
                .gas_limit(5000)
                .transferred_value(0)
                .exec_input(
                    ExecutionInput::new(Selector::new(ink::selector_bytes!("transfer_from")))
                        .push_arg(from)
                        .push_arg(to)
                        .push_arg(amount),
                )
                .returns::<()>()
                .try_invoke();

            match result {
                Ok(Ok(())) => Ok(()),
                _ => Err(Error::TransferFailed),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token_swap::TokenSwap;

    #[ink::test]
    fn new_works() {
        let token_swap = TokenSwap::new();
        assert_eq!(token_swap.swap_count, 0);
    }

    #[ink::test]
    fn create_swap_works() {
        let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();

        let mut token_swap = TokenSwap::new();
        let result = token_swap.create_swap(accounts.alice, accounts.bob, 50, 100, 10);
        assert!(result.is_ok());
        assert_eq!(token_swap.swap_count, 1);
    }

    #[ink::test]
    #[should_panic(expected = "Unauthorized")]
    fn delete_swap_fails_if_not_creator() {
        let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();

        let mut token_swap = TokenSwap::new();
        token_swap
            .create_swap(accounts.alice, accounts.bob, 50, 100, 10)
            .unwrap();

        ink_env::test::set_caller::<ink_env::DefaultEnvironment>(accounts.charlie);
        token_swap
            .delete_swap(0)
            .expect("Expected unauthorized error");
    }

    #[ink::test]
    fn delete_swap_works_if_creator() {
        let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();

        let mut token_swap = TokenSwap::new();
        token_swap
            .create_swap(accounts.alice, accounts.bob, 50, 100, 10)
            .unwrap();
        let result = token_swap.delete_swap(0);
        assert!(result.is_ok());
        assert_eq!(token_swap.swap_count, 0);
    }

    #[ink::test]
    fn accept_swap_works() {
        let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();

        let mut token_swap = TokenSwap::new();
        token_swap
            .create_swap(accounts.alice, accounts.bob, 50, 100, 10)
            .unwrap();
        let result = token_swap.accept_swap(0);
        assert!(result.is_ok());
    }
}
