// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {EasyBank} from "src/EasyBank.sol";
import {Test} from "forge-std/Test.sol";
import {Token} from "src/Token.sol";

contract EasyBankHandler is Test {

    EasyBank public bank;
    Token public token;
    address[] public actors;

    uint256 public ghost_total_deposited;
    uint256 public ghost_total_withdrawn;
    uint256 public ghost_withdrawal_count;
    mapping(address => uint256) public ghost_balance;
    mapping(address => bool) internal _isActor;

    constructor(address _bank, address _token) {
        bank = EasyBank(_bank);
        token = Token(_token);
    }

function actorsLength() external view returns (uint256) {
    return actors.length;
}

function deposit(address actorSeed, uint256 amount) external {
    address actor = address(uint160(uint256(keccak256(abi.encodePacked(actorSeed)))));
    if (actor == address(0)) actor = address(0x1);
    uint256 actorTokenBalance = token.balanceOf(actor);
    if (actorTokenBalance == 0) {
        deal(address(token), actor, 1e18);
        actorTokenBalance = 1e18;
    }
    amount = bound(amount, 1, actorTokenBalance > type(uint128).max ? type(uint128).max : actorTokenBalance);
    vm.startPrank(actor);
    token.approve(address(bank), amount);
    bank.deposit(amount);
    vm.stopPrank();
    ghost_total_deposited += amount;
    ghost_balance[actor] += amount;
    if (!_isActor[actor]) {
        _isActor[actor] = true;
        actors.push(actor);
    }
}

function withdraw(address actorSeed, uint256 amount) external {
    address actor = address(uint160(uint256(keccak256(abi.encodePacked(actorSeed)))));
    if (actor == address(0)) actor = address(0x1);
    if (ghost_balance[actor] == 0) return;
    amount = bound(amount, 1, ghost_balance[actor]);
    vm.startPrank(actor);
    bank.withdraw(amount);
    vm.stopPrank();
    ghost_total_withdrawn += amount;
    ghost_withdrawal_count += 1;
    ghost_balance[actor] -= amount;
}

}