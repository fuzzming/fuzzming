// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test} from "forge-std/Test.sol";
import {Vault} from "../../../src/Vault.sol";
import {Token} from "../../../src/Token.sol";

contract VaultHandler is Test {
    Vault public vault;
    Token public token;

    uint256 public ghost_totalDeposits;
    uint256 public ghost_totalWithdrawals;

    address[] internal actors;
    address internal currentActor;

    constructor(Vault _vault, Token _token) {
        vault = _vault;
        token = _token;
        actors.push(makeAddr("actor0"));
        actors.push(makeAddr("actor1"));
        actors.push(makeAddr("actor2"));
    }

    modifier useActor(uint256 seed) {
        currentActor = actors[seed % actors.length];
        vm.startPrank(currentActor);
        _;
        vm.stopPrank();
    }

    function handler_deposit(uint256 actorSeed, uint256 amount) external useActor(actorSeed) {
        uint256 remaining = vault.depositCap() - vault.totalAssets();
        if (remaining == 0) return;
        amount = bound(amount, 1, remaining);
        token.mint(currentActor, amount);
        token.approve(address(vault), amount);
        vault.deposit(amount);
        ghost_totalDeposits += amount;
    }

    function handler_withdraw(uint256 actorSeed, uint256 shares) external useActor(actorSeed) {
        uint256 bal = vault.balanceOf(currentActor);
        if (bal == 0) return;
        shares = bound(shares, 1, bal);
        vault.withdraw(shares);
        ghost_totalWithdrawals += shares;
    }
}
