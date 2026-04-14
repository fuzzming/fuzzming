// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test}         from "forge-std/Test.sol";
import {Vault}        from "../../src/Vault.sol";
import {ERC20Mock}    from "openzeppelin/mocks/ERC20Mock.sol";
import {VaultHandler} from "../handlers/VaultHandler.sol";

contract VaultInvariantTest is Test {

    Vault        public vault;
    ERC20Mock    public token;
    VaultHandler public handler;

    function setUp() public {
        token   = new ERC20Mock();
        vault   = new Vault(address(token));
        handler = new VaultHandler(vault, token);
        targetContract(address(handler));
    }

    function invariant_totalAssetsMatch() external view {
        assertEq(
            vault.totalAssets(),
            handler.ghost_totalDeposits() - handler.ghost_totalWithdrawals(),
            "totalAssets diverged"
        );
    }

    function invariant_depositCapRespected() external view {
        assertLe(
            vault.totalAssets(),
            vault.depositCap(),
            "depositCap exceeded"
        );
    }

}