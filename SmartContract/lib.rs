#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "std"), no_main)]

#[ink::contract]
mod token_swap {
    use ink::storage::Mapping;
    use ink::LangError;
    use ink_env::call::{build_call, ExecutionInput, Selector};
    use ink_env::DefaultEnvironment;

    // just use a struct here
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

        // How do you create a swap? should this be a message?
        // you should add some doc comments on top of all the functions, so that the reader can
        // figure out what they are doing
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

            // b is the token you want to receive ? So you probably don't need to check the
            // balance, since you don't have it yet
            let balance_b: Balance = self.get_balance(token_b, caller)?;
            if balance_b < amount_b {
                return Err(Error::InsufficientBalance);
            }

            self.transfer_token(token_a, caller, self.env().account_id(), amount_a)?;

            let expiration = self
                .env()
                .block_number()
                .checked_add(duration)
                .ok_or(Error::CallFailed)?;

            // use a named type and a new fn
            let new_swap = (caller, token_a, token_b, amount_a, amount_b, expiration);

            // use an insert fn on the (to be created) Swap struct
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
