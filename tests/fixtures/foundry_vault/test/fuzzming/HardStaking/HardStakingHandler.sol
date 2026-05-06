// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {HardStaking} from "src/HardStaking.sol";
import {Test} from "forge-std/Test.sol";
import {Token} from "src/Token.sol";

contract HardStakingHandler is Test {

    HardStaking public staking;
    Token public stakingToken;
    Token public rewardToken;
    address[] public actors;
    address public currentActor;
    uint256 public constant INITIAL_BALANCE = 1_000_000e18;

    uint256 public ghost_rewards_added;
    uint256 public ghost_rewards_claimed;
    uint256 public ghost_total_staked;
    mapping(address => uint256) public ghost_staked;
    mapping(address => uint256) public ghost_staking_deposited;
    mapping(address => uint256) public ghost_staking_withdrawn;
    mapping(address => bool) internal _isActor;

    constructor(HardStaking _staking, Token _stakingToken, Token _rewardToken) {
        staking = _staking;
        stakingToken = _stakingToken;
        rewardToken = _rewardToken;
    }

function _addActorIfNew(address actor) internal {
    if (!_isActor[actor]) {
        _isActor[actor] = true;
        actors.push(actor);
        deal(address(stakingToken), actor, INITIAL_BALANCE);
        deal(address(rewardToken), actor, INITIAL_BALANCE);
    }
}

function _selectActor(uint256 seed) internal returns (address) {
    if (actors.length == 0) {
        address newActor = makeAddr("actor0");
        _isActor[newActor] = true;
        actors.push(newActor);
        deal(address(stakingToken), newActor, INITIAL_BALANCE);
        deal(address(rewardToken), newActor, INITIAL_BALANCE);
    }
    uint256 idx = bound(seed, 0, actors.length - 1);
    return actors[idx];
}

function actorsLength() external view returns (uint256) {
    return actors.length;
}

function handler_addNewActor(uint256 seed) external {
    address newActor = makeAddr(string(abi.encodePacked("actor_new", seed, block.timestamp)));
    if (!_isActor[newActor]) {
        _isActor[newActor] = true;
        actors.push(newActor);
        deal(address(stakingToken), newActor, INITIAL_BALANCE);
        deal(address(rewardToken), newActor, INITIAL_BALANCE);
    }
}

function handler_addRewards(uint256 seed, uint256 amount) external {
    address actor = _selectActor(seed);
    _addActorIfNew(actor);
    uint256 balance = rewardToken.balanceOf(actor);
    if (balance == 0) return;
    amount = bound(amount, 1, balance > type(uint128).max ? type(uint128).max : balance);
    vm.startPrank(actor);
    rewardToken.approve(address(staking), amount);
    staking.addRewards(amount);
    vm.stopPrank();
    ghost_rewards_added += amount;
}

function handler_claimRewards(uint256 seed) external {
    address actor = _selectActor(seed);
    _addActorIfNew(actor);
    uint256 balanceBefore = rewardToken.balanceOf(actor);
    vm.startPrank(actor);
    staking.claimRewards();
    vm.stopPrank();
    uint256 balanceAfter = rewardToken.balanceOf(actor);
    uint256 actualClaimed = balanceAfter - balanceBefore;
    ghost_rewards_claimed += actualClaimed;
}

function handler_stake(uint256 seed, uint256 amount) external {
    address actor = _selectActor(seed);
    _addActorIfNew(actor);
    uint256 balance = stakingToken.balanceOf(actor);
    if (balance == 0) return;
    amount = bound(amount, 1, balance > type(uint128).max ? type(uint128).max : balance);
    vm.startPrank(actor);
    stakingToken.approve(address(staking), amount);
    staking.stake(amount);
    vm.stopPrank();
    ghost_staked[actor] += amount;
    ghost_total_staked += amount;
    ghost_staking_deposited[actor] += amount;
}

function handler_unstake(uint256 seed, uint256 amount) external {
    address actor = _selectActor(seed);
    _addActorIfNew(actor);
    uint256 staked = staking.stakedOf(actor);
    if (staked == 0) return;
    amount = bound(amount, 1, staked);
    vm.startPrank(actor);
    staking.unstake(amount);
    vm.stopPrank();
    ghost_staked[actor] -= amount;
    ghost_total_staked -= amount;
    ghost_staking_withdrawn[actor] += amount;
}

}