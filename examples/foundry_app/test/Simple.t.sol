// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.0;

import "forge-std/Test.sol";
import "../src/Simple.sol"; // Import your contract

contract SimpleTest is Test {
    Simple simple;

    function setUp() public {
        simple = new Simple();
    }

    // A standard unit test
    function test_InitialState() public {
        assertEq(simple.total(), 0);
    }

    // A FUZZ test automatically run by Foundry
    // Foundry will pass random `amount` values to fuzz this function
    function testFuzz_Deposit(uint256 amount) public {
        // Prevent random values that overflow our uint256 variable in the contract addition
        vm.assume(amount < type(uint256).max / 2);
        
        uint256 oldTotal = simple.total();
        
        simple.deposit(amount);
        
        assertEq(simple.total(), oldTotal + amount);
    }

    // Fuzz test for the withdraw logic
    function testFuzz_Withdraw(uint256 depositAmount, uint256 withdrawAmount) public {
        vm.assume(depositAmount < type(uint256).max / 2);
        // Withdraw shouldn't exceed deposited amount to test successful execution
        vm.assume(depositAmount >= withdrawAmount);

        simple.deposit(depositAmount);
        
        uint256 beforeWithdraw = simple.total();
        
        simple.withdraw(withdrawAmount);
        
        // Assert the total properly shrunk
        if (withdrawAmount > 0) {
            assertEq(simple.total(), beforeWithdraw - withdrawAmount);
        }
    }
}
