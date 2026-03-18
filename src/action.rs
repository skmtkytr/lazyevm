use crate::config::TokenEntry;
use crate::panels::PanelId;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Action {
    // System
    Tick,
    Render,
    Quit,
    None,

    // Navigation
    SwitchPanel(PanelId),
    FocusSidebar,
    FocusContent,
    Up,
    Down,
    Left,
    Right,
    Select,
    Back,
    NextTab,
    PrevTab,

    // Status
    SetStatus(String),
    ClearStatus,
    Error(String),

    // Wallet actions
    RefreshWallets,
    WalletsLoaded(Vec<WalletEntry>),
    CreateWallet,
    WalletCreated(String),
    ImportWallet,
    WalletImported(String),
    BalanceLoaded {
        address: String,
        balance: String,
    },
    UnlockWallet {
        name: String,
        password: String,
    },
    WalletAddressResolved {
        name: String,
        address: String,
    },

    // Network actions
    SelectNetwork(String),
    NetworkChanged {
        name: String,
        url: String,
    },
    AddNetwork {
        name: String,
        url: String,
    },
    RemoveNetwork(String),

    // Anvil actions
    StartAnvil,
    StopAnvil,
    AnvilStarted {
        port: u16,
    },
    AnvilStopped,
    AnvilLog(String),
    AnvilAccounts(Vec<AnvilAccount>),
    MineBlock,
    BlockMined(u64),
    ResetAnvil,
    AnvilTransfer {
        from_key: String,
        to: String,
        value: String,
        token: Option<String>,
    },
    AnvilTransferDone(String),
    AnvilDumpState,
    AnvilLoadState,
    AnvilError(String),

    // Fork actions
    StartAnvilFork {
        fork_url: String,
    },
    SetForkUrl(String),

    // Token actions
    RefreshTokenBalances {
        account: String,
    },
    TokenBalancesLoaded {
        account: String,
        balances: Vec<TokenBalance>,
    },
    AddCustomToken {
        address: String,
    },
    CustomTokenResolved(TokenEntry),
    DealToken {
        token_address: String,
        to: String,
        amount: String,
        decimals: u8,
        balance_slot: u64,
    },
    DealTokenDone(String),
    DealEth {
        to: String,
        amount: String,
    },
    DealEthDone(String),
    DetectBalanceSlot {
        token_address: String,
        test_account: String,
    },
    BalanceSlotDetected {
        token_address: String,
        slot: u64,
    },
    RemoveToken(String),

    // Forge actions
    ForgeBuild,
    ForgeTest,
    ForgeScript(String),
    ForgeOutput(String),
    ForgeDone {
        success: bool,
        summary: String,
    },
    ForgeClear,

    // Cast actions
    CastCall {
        to: String,
        sig: String,
        args: Vec<String>,
    },
    CastSend {
        to: String,
        sig: String,
        args: Vec<String>,
    },
    CastBalance(String),
    CastResult(String),
    CastError(String),

    // Explorer actions
    RefreshBlocks,
    BlocksLoaded(Vec<BlockInfo>),
    SelectBlock(u64),
    TxsLoaded(Vec<TxInfo>),
    SelectTx(String),
    TxDetailLoaded(TxDetail),
}

#[derive(Debug, Clone)]
pub struct WalletEntry {
    pub name: String,
    pub address: String,
    pub balance: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AnvilAccount {
    pub address: String,
    pub key: String,
    pub balance: String,
}

#[derive(Debug, Clone)]
pub struct TokenBalance {
    pub token_address: String,
    pub symbol: String,
    pub balance: String,
    pub raw_balance: String,
    pub status: TokenBalanceStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenBalanceStatus {
    Unknown,
    Loading,
    Loaded,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct BlockInfo {
    pub number: u64,
    pub hash: String,
    pub timestamp: u64,
    pub tx_count: usize,
    pub gas_used: String,
}

#[derive(Debug, Clone)]
pub struct TxInfo {
    pub hash: String,
    pub from: String,
    pub to: String,
    pub value: String,
    pub status: bool,
}

#[derive(Debug, Clone)]
pub struct TxDetail {
    pub hash: String,
    pub from: String,
    pub to: String,
    pub value: String,
    pub gas_used: String,
    pub gas_price: String,
    pub input: String,
    pub block_number: u64,
    pub status: bool,
}
