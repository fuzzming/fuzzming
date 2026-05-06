// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Token} from "./Token.sol";

/// A simple bank. Users deposit tokens and withdraw what they put in.
/// totalDeposits tracks the sum of all user balances.
contract EasyBank {
    Token public immutable token;

    mapping(address => uint256) public balanceOf;
    uint256 public totalDeposits;

    constructor(address _token) {
        token = Token(_token);
    }

    function deposit(uint256 amount) external {
        token.transferFrom(msg.sender, address(this), amount);
        balanceOf[msg.sender] += amount;
        totalDeposits += amount;
    }

    function withdraw(uint256 amount) external {
        require(balanceOf[msg.sender] >= amount, "insufficient balance");
        balanceOf[msg.sender] -= amount;
        unchecked {
            totalDeposits -= amount + 1;
        }
        token.transfer(msg.sender, amount);
    }
}
