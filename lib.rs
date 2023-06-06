#![cfg_attr(not(feature = "std"), no_std)]

use ink_lang as ink;

#[ink::contract]
mod token_swap {
    use ink_env::{call::FromAccountId, DefaultEnvironment, EmitEvent, Env};
    use ink_prelude::collections::BTreeMap;
    use ink_storage::{
        traits::{PackedLayout, SpreadLayout},
        Lazy,
    };

    #[derive(Debug, Clone, PartialEq, Eq, PackedLayout, SpreadLayout)]
    #[cfg_attr(feature = "std", derive(::scale_info::TypeInfo))]
    pub struct Swap {
        pub creator: AccountId,
        pub token_a: AccountId,
        pub token_b: AccountId,
        pub amount_a: Balance,
        pub amount_b: Balance,
        pub expiration: Timestamp,
    }

    #[ink(storage)]
    pub struct TokenSwap {
        swaps: BTreeMap<Hash, Swap>,
        swap_count: u64,
    }

    impl TokenSwap {
        #[ink(constructor)]
        pub fn new() -> Self {
            Self {
                swaps: BTreeMap::new(),
                swap_count: 0,
            }
        }

        #[ink(message)]
        pub fn create_swap(
            &mut self,
            token_a: AccountId,
            token_b: AccountId,
            amount_a: Balance,
            amount_b: Balance,
            duration: Timestamp,
        ) -> Hash {
            let caller = self.env().caller();
            let balance_a: Balance = ink_env::call::build_call::<DefaultEnvironment>()
                .callee(token_a)
                .gas_limit(5000)
                .transferred_value(0)
                .exec_input(
                    ExecutionInput::new(Selector::new(ink::selector_bytes!("balance_of")))
                        .push_arg(caller),
                )
                .returns::<Balance>()
                .fire()
                .unwrap_or_default();

            assert!(balance_a >= amount_a, "Insufficient balance for token A");

            let swap_id = {
                let mut id_data = [0; 32];
                let count_as_bytes = self.swap_count.to_be_bytes();
                id_data[..8].copy_from_slice(&count_as_bytes);
                Hash::from(id_data)
            };
            

            let new_swap = Swap {
                creator: caller,
                token_a,
                token_b,
                amount_a,
                amount_b,
                expiration: self.env().block_timestamp() + duration,
            };

            self.swaps.insert(swap_id, new_swap);
            self.swap_count += 1;

            swap_id
        }

        #[ink(message)]
        pub fn delete_swap(&mut self, swap_id: Hash) {
            let caller = self.env().caller();

            let swap = self
                .swaps
                .get(&swap_id)
                .expect("Swap not found");

            assert!(caller == swap.creator, "Only the creator of the swap can delete it.");

            // Remove swap from the list
            self.swaps.remove(&swap_id);
        }

        #[ink(message)]
        pub fn accept_swap(&mut self, swap_id: Hash) {
            let swap = self
                .swaps
                .get(&swap_id)
                .expect("Swap not found");

            // Check everything is correct 

            let caller = self.env().caller();
            let balance_b: Balance = ink_env::call::build_call::<DefaultEnvironment>()
                .callee(swap.token_b)
                .gas_limit(5000)
                .transferred_value(0)
                .exec_input(
                    ExecutionInput::new(Selector::new(ink::selector_bytes!("balance_of")))
                        .push_arg(caller),
                )
                .returns::<Balance>()
                .fire()
                .unwrap_or_default();

            assert!(
                balance_b >= swap.amount_b,
                "Insufficient balance for token B"
            );

            let balance_a: Balance = ink_env::call::build_call::<DefaultEnvironment>()
            .callee(swap.token_a)
            .gas_limit(5000)
            .transferred_value(0)
            .exec_input(
                ExecutionInput::new(Selector::new(ink::selector_bytes!("balance_of")))
                    .push_arg(caller),
            )
            .returns::<Balance>()
            .fire()
            .unwrap_or_default();

            assert!(balance_a >= amount_a, "Insufficient balance for token A");
            

            let current_time = self.env().block_timestamp();
            assert!(
                current_time <= swap.expiration,
                "Swap has already expired"
            );

            // Transfer tokens between users

            ink_env::call::build_call::<DefaultEnvironment>()
            .callee(swap.token_a)
            .gas_limit(5000)
            .transferred_value(0)
            .exec_input(
                ExecutionInput::new(Selector::new(ink::selector_bytes!("transfer_from")))
                    .push_arg(self.env().caller())
                    .push_arg(swap.token_b)
                    .push_arg(swap.amount_a),
            )
            .returns::<()>()
            .fire()
            .unwrap_or_default();

        ink_env::call::build_call::<DefaultEnvironment>()
            .callee(swap.token_b)
            .gas_limit(5000)
            .transferred_value(0)
            .exec_input(
                ExecutionInput::new(Selector::new(ink::selector_bytes!("transfer_from")))
                    .push_arg(caller)
                    .push_arg(swap.token_a)
                    .push_arg(swap.amount_b),
            )
            .returns::<()>()
            .fire()
            .unwrap_or_default();

        // Remove swap from the list after transfered assets
        self.swaps.remove(&swap_id);
    }
    }
}


        
