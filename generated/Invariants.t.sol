pragma solidity ^0.8.20;
import "forge-std/Test.sol";
import "./Vault.sol";
import "./VaultHandler.sol";

contract VaultInvariantTest is Test {
    Vault public target;
    VaultHandler public handler;

    function setUp() public {
        target = new Vault();
        handler = new VaultHandler(target);
    }

    function invariant_balances_non_negative() public view {
        for (address actor in target.balances) {
            assertGt(target.balances(actor), 0);
        }
    }

    function invariant_total_supply() public view {
        uint256 totalSupply = 0;
        for (address actor in target.balances) {
            totalSupply += target.balances(actor);
        }
        assertEq(address(target).balance, totalSupply);
    }

    function test_invariant_balances_non_negative() public {
        handler.deposit(address(0x1337), 100 ether);
        invariant_balances_non_negative();
    }

    function test_invariant_total_supply() public {
        handler.deposit(address(0x1337), 100 ether);
        invariant_total_supply();
    }

    function test_invariant_total_supply_after_withdrawal() public {
        handler.deposit(address(0x1337), 100 ether);
        handler.withdraw(address(0x1337), 50 ether);
        invariant_total_supply();
    }
}