// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test} from "forge-std/Test.sol";
import {Vault} from "../../../src/Vault.sol";
import {Token} from "../../../src/Token.sol";
import {VaultHandler} from "./VaultHandler.sol";

contract VaultInvariantTest is Test {
    Vault public vault;
    Token public token;
    VaultHandler public handler;

    function setUp() external {
        token = new Token();
        vault = new Vault(address(token));
        handler = new VaultHandler(vault, token);
        targetContract(address(handler));
    }

    function invariant_totalAssetsMatch() external view {
        assertEq(
            vault.totalAssets(),
            handler.ghost_totalDeposits() - handler.ghost_totalWithdrawals(),
            "totalAssets diverged from ghost tracking"
        );
    }

    function invariant_depositCapRespected() external view {
        assertLe(
            vault.totalAssets(),
            vault.depositCap(),
            "deposit cap exceeded"
        );
    }
}
