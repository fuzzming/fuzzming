// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {MediumVaultHandler} from "./MediumVaultHandler.sol";
import {Test} from "forge-std/Test.sol";
import {MediumVault} from "src/MediumVault.sol";
import {Token} from "src/Token.sol";

contract MediumVaultInvariantTest is Test {

    MediumVault public vault;
    Token public token;
    address public feeCollector;
    MediumVaultHandler public handler;
    address[] internal actorAddresses;
    uint256 internal constant INITIAL_ACTOR_BALANCE = 1_000_000e18;

    function setUp() public {
        feeCollector = makeAddr("feeCollector");
        token = new Token();
        vault = new MediumVault(address(token), feeCollector);
        handler = new MediumVaultHandler(address(vault), address(token), feeCollector);
        actorAddresses = new address[](5);
        actorAddresses[0] = makeAddr("actor0");
        actorAddresses[1] = makeAddr("actor1");
        actorAddresses[2] = makeAddr("actor2");
        actorAddresses[3] = makeAddr("actor3");
        actorAddresses[4] = makeAddr("actor4");
        for (uint256 i = 0; i < actorAddresses.length; i++) {
            deal(address(token), actorAddresses[i], INITIAL_ACTOR_BALANCE);
        }
        bytes4[] memory selectors = new bytes4[](2);
        selectors[0] = MediumVaultHandler.deposit.selector;
        selectors[1] = MediumVaultHandler.withdraw.selector;
        targetSelector(FuzzSelector({addr: address(handler), selectors: selectors}));
        targetContract(address(handler));
        for (uint256 i = 0; i < actorAddresses.length; i++) {
            targetSender(actorAddresses[i]);
        }
    }

function invariant_actualBalanceVsTotalAssets() external view {
    uint256 actualBalance = token.balanceOf(address(vault));
    uint256 reportedAssets = vault.totalAssets();
    // The fee bug: withdraw reduces totalAssets by (amount - fee) instead of amount,
    // so reportedAssets grows above actualBalance after each withdrawal.
    assert(actualBalance == reportedAssets);
}

function invariant_emptyVaultConsistency() external view {
    if (vault.totalShares() == 0) {
        // If no shares exist, totalAssets should also be zero.
        // The fee bug can leave totalAssets > 0 when totalShares == 0.
        assert(vault.totalAssets() == 0);
    }
}

function invariant_noActorOwnsMoreThanTotal() external view {
    uint256 len = handler.actorsLength();
    uint256 total = vault.totalShares();
    for (uint256 i = 0; i < len; i++) {
        address actor = handler.actors(i);
        assert(vault.sharesOf(actor) <= total);
    }
}

function invariant_tokenConservation() external view {
    uint256 vaultBalance = token.balanceOf(address(vault));
    uint256 feeCollectorBalance = token.balanceOf(feeCollector);
    // All deposited tokens must be accounted for: either in vault or collected as fees.
    // The fee bug causes totalAssets to drift above actual vault balance.
    assert(vaultBalance + feeCollectorBalance == handler.ghost_total_deposited());
}

function invariant_totalSharesConsistency() external view {
    uint256 sumShares = 0;
    uint256 len = handler.actorsLength();
    for (uint256 i = 0; i < len; i++) {
        address actor = handler.actors(i);
        sumShares += vault.sharesOf(actor);
    }
    assert(sumShares == vault.totalShares());
}

function invariant_vaultSolvency() external view {
    // If there are outstanding shares, the vault must hold enough tokens to cover totalAssets.
    // Due to the fee bug, totalAssets > actual balance, so the vault is insolvent.
    if (vault.totalShares() > 0) {
        uint256 actualBalance = token.balanceOf(address(vault));
        uint256 reportedAssets = vault.totalAssets();
        // Vault must be able to cover what it claims to hold.
        assert(actualBalance >= reportedAssets);
    }
}

}