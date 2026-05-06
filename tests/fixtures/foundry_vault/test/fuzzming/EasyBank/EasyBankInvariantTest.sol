// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {EasyBankHandler} from "./EasyBankHandler.sol";
import {Test} from "forge-std/Test.sol";
import {EasyBank} from "src/EasyBank.sol";
import {Token} from "src/Token.sol";

contract EasyBankInvariantTest is Test {

    EasyBank public bank;
    Token public token;
    EasyBankHandler public handler;

    function setUp() public {
        token = new Token();
        bank = new EasyBank(address(token));
        handler = new EasyBankHandler(address(bank), address(token));
        bytes4[] memory selectors = new bytes4[](2);
        selectors[0] = EasyBankHandler.deposit.selector;
        selectors[1] = EasyBankHandler.withdraw.selector;
        targetSelector(FuzzSelector({addr: address(handler), selectors: selectors}));
        targetContract(address(handler));
    }

function invariant_individualGhostBalancesMatch() external view {
    uint256 len = handler.actorsLength();
    for (uint256 i = 0; i < len; i++) {
        address actor = handler.actors(i);
        assertEq(
            bank.balanceOf(actor),
            handler.ghost_balance(actor),
            "bank.balanceOf(actor) != ghost_balance[actor]"
        );
    }
}

function invariant_tokenBalanceEqualsTotalDeposits() external view {
    assertEq(
        token.balanceOf(address(bank)),
        bank.totalDeposits(),
        "token.balanceOf(bank) != bank.totalDeposits()"
    );
}

function invariant_totalDepositsEqualsGhostNet() external view {
    uint256 ghostNet = handler.ghost_total_deposited() - handler.ghost_total_withdrawn();
    assertEq(
        bank.totalDeposits(),
        ghostNet,
        "totalDeposits != ghost_total_deposited - ghost_total_withdrawn"
    );
}

function invariant_totalDepositsEqualsSumOfBalances() external view {
    uint256 sumBalances = 0;
    uint256 len = handler.actorsLength();
    for (uint256 i = 0; i < len; i++) {
        address actor = handler.actors(i);
        sumBalances += bank.balanceOf(actor);
    }
    assertEq(
        bank.totalDeposits(),
        sumBalances,
        "totalDeposits != sum of all actor balances"
    );
}

function invariant_totalDepositsNoUnderflow() external view {
    assertLe(
        bank.totalDeposits(),
        handler.ghost_total_deposited(),
        "totalDeposits overflowed above total deposited"
    );
}

}