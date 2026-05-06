// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {MediumVault} from "src/MediumVault.sol";
import {Test} from "forge-std/Test.sol";
import {Token} from "src/Token.sol";

contract MediumVaultHandler is Test {

    MediumVault public vault;
    Token public token;
    address public feeCollector;
    address[] public actors;
    mapping(address => bool) public isActor;

    uint256 public ghost_total_deposited;
    mapping(address => uint256) public ghost_actor_deposits;

    constructor(address _vault, address _token, address _feeCollector) {
        vault = MediumVault(_vault);
        token = Token(_token);
        feeCollector = _feeCollector;
    }

function actorsLength() external view returns (uint256) {
    return actors.length;
}

function deposit(uint256 amount) external {
    address actor = msg.sender;
    uint256 actorBalance = token.balanceOf(actor);
    if (actorBalance == 0) return;
    amount = bound(amount, 1, actorBalance > type(uint128).max ? type(uint128).max : actorBalance);
    vm.startPrank(actor);
    token.approve(address(vault), amount);
    vault.deposit(amount);
    vm.stopPrank();
    ghost_total_deposited += amount;
    ghost_actor_deposits[actor] += amount;
    if (!isActor[actor]) {
        isActor[actor] = true;
        actors.push(actor);
    }
}

function withdraw(uint256 shares) external {
    address actor = msg.sender;
    uint256 actorShares = vault.sharesOf(actor);
    if (actorShares == 0) return;
    shares = bound(shares, 1, actorShares);
    vm.startPrank(actor);
    vault.withdraw(shares);
    vm.stopPrank();
    if (!isActor[actor]) {
        isActor[actor] = true;
        actors.push(actor);
    }
}

}