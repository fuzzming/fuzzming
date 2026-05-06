// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Token} from "./Token.sol";

/// A share-based vault with a 1% withdrawal fee sent to a fee collector.
/// Shares are proportional to the vault's total assets at deposit time.
contract MediumVault {
    Token public immutable token;
    address public immutable feeCollector;

    mapping(address => uint256) public sharesOf;
    uint256 public totalShares;
    uint256 public totalAssets;

    uint256 public constant FEE_BPS = 100; // 1%

    constructor(address _token, address _feeCollector) {
        token = Token(_token);
        feeCollector = _feeCollector;
    }

    function deposit(uint256 amount) external returns (uint256 shares) {
        shares = totalShares == 0 ? amount : (amount * totalShares) / totalAssets;
        require(shares > 0, "zero shares");
        token.transferFrom(msg.sender, address(this), amount);
        sharesOf[msg.sender] += shares;
        totalShares += shares;
        totalAssets += amount;
    }

    function withdraw(uint256 shares) external returns (uint256 amount) {
        require(sharesOf[msg.sender] >= shares, "insufficient shares");
        amount = (shares * totalAssets) / totalShares;
        require(amount > 0, "zero amount");
        uint256 fee = (amount * FEE_BPS) / 10_000;
        sharesOf[msg.sender] -= shares;
        totalShares -= shares;
        totalAssets -= (amount - fee);
        token.transfer(feeCollector, fee);
        token.transfer(msg.sender, amount - fee);
    }
}
