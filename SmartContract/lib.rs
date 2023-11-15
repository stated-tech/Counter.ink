#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "std"), no_main)]

#[ink::contract]
mod token_swap {
    use ink::storage::Mapping;
    use ink::LangError;
    use ink_env::call::{build_call, ExecutionInput, Selector};
    use ink_env::DefaultEnvironment;

    pub type Swap = (
        AccountId,
        AccountId,
        AccountId,
        Balance,
        Balance,
        BlockNumber,
        Balance,           // Amount of Token A already accepted
        Balance,           // Amount of Token B already accepted
        Option<AccountId>, // Allowed acceptor
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
        DelegateFailed,
        DelegateFunctionFailed,
    }

    pub type Result<T> = core::result::Result<T, Error>;

    #[ink(storage)]
    pub struct TokenSwap {
        pub swaps: Mapping<u64, Swap>,
        pub swap_count: u64,
        delegated_contract: Option<AccountId>,
        owner: AccountId,
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
                delegated_contract: None,
                owner: Self::env().caller(),
            }
        }

        #[ink(message)]
        pub fn set_delegated_contract(&mut self, contract: AccountId) {
            if self.env().caller() != self.owner {
                ink_env::debug_println!("Unauthorized");
                return;
            }
            self.delegated_contract = Some(contract);
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
            allowed_acceptor: Option<AccountId>, // Nouvel argument
        ) -> Result<u64> {
            if let Some(delegate) = self.delegated_contract {
                let selector = ink::selector_bytes!("create_swap_delegate");
                let nested_result: core::result::Result<
                    core::result::Result<(), LangError>,
                    ink_env::Error,
                > = build_call::<DefaultEnvironment>()
                    .call(delegate)
                    .gas_limit(5000)
                    .transferred_value(0)
                    .exec_input(
                        ExecutionInput::new(Selector::new(selector))
                            .push_arg(token_a)
                            .push_arg(token_b)
                            .push_arg(amount_a)
                            .push_arg(amount_b)
                            .push_arg(duration),
                    )
                    .returns::<()>()
                    .try_invoke();

                let result = match nested_result {
                    Ok(inner_result) => inner_result.map_err(|_| Error::DelegateFunctionFailed),
                    Err(_) => Err(Error::DelegateFailed),
                };

                match result {
                    Ok(()) => Ok(self.swap_count),
                    Err(e) => Err(e),
                }
            } else {
                let caller = self.env().caller();
                let balance_a: Balance = self.get_balance(token_a, caller)?;

                if balance_a < amount_a {
                    return Err(Error::InsufficientBalance);
                }

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

                let new_swap = (
                    caller,
                    token_a,
                    token_b,
                    amount_a,
                    amount_b,
                    expiration,
                    0,
                    0,
                    allowed_acceptor,
                );

                self.swaps.insert(self.swap_count, &new_swap);
                let id = self.swap_count;
                self.swap_count = self.swap_count.checked_add(1).ok_or(Error::CallFailed)?;

                self.env().emit_event(SwapCreated {
                    id,
                    creator: caller,
                });

                Ok(id)
            }
        }

        #[ink(message)]
        pub fn delete_swap(&mut self, swap_id: u64) -> Result<()> {
            if !self.swaps.contains(&swap_id) {
                return Err(Error::SwapNotFound);
            }

            let swap_data = self.swaps.get(&swap_id).unwrap();
            let creator = swap_data.0;

            if self.env().caller() != creator {
                return Err(Error::Unauthorized);
            }

            self.swaps.remove(&swap_id);

            self.env().emit_event(SwapDeleted { id: swap_id });

            Ok(())
        }

        fn transfer_token(
            &self,
            token_contract: AccountId,
            from: AccountId,
            to: AccountId,
            amount: Balance,
        ) -> Result<()> {
            let transfer_result: core::result::Result<
                core::result::Result<(), LangError>,
                ink_env::Error,
            > = build_call::<DefaultEnvironment>()
                .call(token_contract)
                .gas_limit(5000)
                .transferred_value(0)
                .exec_input(
                    ExecutionInput::new(Selector::new(ink::selector_bytes!("transfer")))
                        .push_arg(from)
                        .push_arg(to)
                        .push_arg(amount),
                )
                .returns::<()>()
                .try_invoke();

            match transfer_result {
                Ok(Ok(())) => Ok(()),
                Ok(Err(_)) => Err(Error::TransferFailed),
                Err(_) => Err(Error::CallFailed),
            }
        }

        #[ink(message)]
        pub fn accept_swap(
            &mut self,
            swap_id: u64,
            amount_a: Balance,
            amount_b: Balance,
        ) -> Result<()> {
            if !self.swaps.contains(&swap_id) {
                return Err(Error::SwapNotFound);
            }

            let swap_data = self.swaps.get(&swap_id).unwrap();

            let creator = swap_data.0;
            let token_a = swap_data.1;
            let token_b = swap_data.2;
            let required_a = swap_data.3;
            let required_b = swap_data.4;
            let expiration = swap_data.5;
            let accepted_a = swap_data.6;
            let accepted_b = swap_data.7;

            if let Some(allowed_acceptor) = swap_data.8 {
                if self.env().caller() != allowed_acceptor {
                    return Err(Error::Unauthorized);
                }
            }

            if self.env().block_number() > expiration {
                return Err(Error::SwapExpired);
            }

            if amount_a + accepted_a > required_a || amount_b + accepted_b > required_b {
                return Err(Error::InsufficientBalance);
            }

            self.transfer_token(token_a, self.env().caller(), creator, amount_a)?;
            self.transfer_token(token_b, self.env().caller(), creator, amount_b)?;

            let allowed_acceptor = swap_data.8;

            let updated_swap = (
                creator,
                token_a,
                token_b,
                required_a,
                required_b,
                expiration,
                accepted_a + amount_a,
                accepted_b + amount_b,
                allowed_acceptor,
            );

            self.swaps.insert(swap_id, &updated_swap);

            self.env().emit_event(SwapAccepted {
                id: swap_id,
                acceptor: self.env().caller(),
            });

            Ok(())
        }
    }
}
