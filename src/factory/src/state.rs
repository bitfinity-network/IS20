use candid::Principal;
use ic_cdk::export::candid::CandidType;
use ic_helpers::factory::{Factory, FactoryConfiguration, FactoryState};
use ic_storage::{stable::Versioned, IcStorage};
use serde::Deserialize;

// 1 ICP
pub const DEFAULT_ICP_FEE: u64 = 10u64.pow(8);

const DEFAULT_LEDGER_PRINCIPAL: &str = "ryjl3-tyaaa-aaaaa-aaaba-cai";

#[derive(CandidType, Deserialize, IcStorage)]
pub struct State {
    pub factory: Factory<String>,
    pub configuration: FactoryConfiguration,
    pub token_wasm: Option<Vec<u8>>,
}

impl State {
    pub fn new(controller: Principal, ledger_principal: Option<Principal>) -> Self {
        let ledger = ledger_principal.unwrap_or_else(|| {
            Principal::from_text(DEFAULT_LEDGER_PRINCIPAL)
                .expect("Const principal value, never fails.")
        });
        Self {
            factory: Default::default(),
            token_wasm: None,
            configuration: FactoryConfiguration::new(
                ledger,
                DEFAULT_ICP_FEE,
                controller,
                controller,
            ),
        }
    }
}

impl Versioned for State {
    type Previous = ();

    fn upgrade((): ()) -> Self {
        Self::default()
    }
}

impl Default for State {
    fn default() -> Self {
        // The default state is only used to initialize storage before `init` method is called, so
        // it does not matter, if the state we create is not valid.
        Self {
            factory: Default::default(),
            token_wasm: None,
            configuration: FactoryConfiguration::new(
                Principal::anonymous(),
                0,
                Principal::anonymous(),
                Principal::anonymous(),
            ),
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub fn get_token_bytecode() -> Vec<u8> {
    State::get()
        .borrow()
        .token_wasm
        .clone()
        .expect("the token bytecode should be set before accessing it")
}

impl FactoryState<String> for State {
    fn factory(&self) -> &Factory<String> {
        &self.factory
    }

    fn factory_mut(&mut self) -> &mut Factory<String> {
        &mut self.factory
    }

    fn configuration(&self) -> &FactoryConfiguration {
        &self.configuration
    }

    fn configuration_mut(&mut self) -> &mut FactoryConfiguration {
        &mut self.configuration
    }
}
