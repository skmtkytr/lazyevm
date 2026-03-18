pub mod anvil;
pub mod cast;
pub mod explorer;
pub mod forge;
pub mod wallets;

use strum::{Display, EnumIter};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, EnumIter)]
pub enum PanelId {
    Wallets,
    Anvil,
    Forge,
    Cast,
    Explorer,
}

impl PanelId {
    pub fn label(&self) -> &'static str {
        match self {
            PanelId::Wallets => "Wallets",
            PanelId::Anvil => "Anvil",
            PanelId::Forge => "Forge",
            PanelId::Cast => "Cast",
            PanelId::Explorer => "Explorer",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            PanelId::Wallets => "◈",
            PanelId::Anvil => "⚒",
            PanelId::Forge => "⚙",
            PanelId::Cast => "⟐",
            PanelId::Explorer => "◎",
        }
    }
}
