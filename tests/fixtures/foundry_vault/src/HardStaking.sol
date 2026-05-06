// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Token} from "./Token.sol";

/// Staking contract: users stake tokens to earn a proportional share of rewards.
/// Anyone can push rewards in via addRewards(). Rewards are split among all stakers
/// proportionally to their stake at the time rewards were added.
contract HardStaking {
    Token public immutable stakingToken;
    Token public immutable rewardToken;

    mapping(address => uint256) public stakedOf;
    uint256 public totalStaked;

    uint256 public rewardPerTokenStored;
    mapping(address => uint256) public userRewardPerTokenPaid;
    mapping(address => uint256) public pendingRewards;

    uint256 public totalRewardsAdded;
    uint256 public totalRewardsClaimed;

    constructor(address _stakingToken, address _rewardToken) {
        stakingToken = Token(_stakingToken);
        rewardToken = Token(_rewardToken);
    }

    function addRewards(uint256 amount) external {
        rewardToken.transferFrom(msg.sender, address(this), amount);
        totalRewardsAdded += amount;
        if (totalStaked > 0) {
            rewardPerTokenStored += (amount * 1e18) / totalStaked;
        }
    }

    function stake(uint256 amount) external {
        stakingToken.transferFrom(msg.sender, address(this), amount);
        stakedOf[msg.sender] += amount;
        totalStaked += amount;
    }

    function unstake(uint256 amount) external {
        _updateReward(msg.sender);
        require(stakedOf[msg.sender] >= amount, "insufficient stake");
        stakedOf[msg.sender] -= amount;
        totalStaked -= amount;
        stakingToken.transfer(msg.sender, amount);
    }

    function claimRewards() external {
        _updateReward(msg.sender);
        uint256 reward = pendingRewards[msg.sender];
        if (reward > 0) {
            pendingRewards[msg.sender] = 0;
            totalRewardsClaimed += reward;
            rewardToken.transfer(msg.sender, reward);
        }
    }

    function earned(address account) public view returns (uint256) {
        return (stakedOf[account] * (rewardPerTokenStored - userRewardPerTokenPaid[account])) / 1e18
            + pendingRewards[account];
    }

    function _updateReward(address account) internal {
        pendingRewards[account] = earned(account);
        userRewardPerTokenPaid[account] = rewardPerTokenStored;
    }
}
