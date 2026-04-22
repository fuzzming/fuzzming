// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Token} from "./Token.sol";

/// Simple single-asset vault.
/// Shares are 1:1 with deposited tokens (no yield, no fees).
contract Vault {
    Token public immutable token;
    uint256 public constant depositCap = 1_000_000e18;

    uint256 public totalAssets;
    mapping(address => uint256) public balanceOf;

    constructor(address _token) {
        token = Token(_token);
    }

    function deposit(uint256 amount) external returns (uint256 shares) {
        require(totalAssets + amount <= depositCap, "cap exceeded");
        token.transferFrom(msg.sender, address(this), amount);
        shares = amount;
        balanceOf[msg.sender] += shares;
        totalAssets += amount;
    }

    function withdraw(uint256 shares) external {
        require(balanceOf[msg.sender] >= shares, "insufficient shares");
        balanceOf[msg.sender] -= shares;
        totalAssets -= shares;
        token.transfer(msg.sender, shares);
    }
}
