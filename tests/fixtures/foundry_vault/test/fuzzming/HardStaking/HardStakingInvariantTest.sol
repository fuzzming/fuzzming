// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {HardStakingHandler} from "./HardStakingHandler.sol";
import {Test} from "forge-std/Test.sol";
import {HardStaking} from "src/HardStaking.sol";
import {Token} from "src/Token.sol";

contract HardStakingInvariantTest is Test {

    HardStaking public staking;
    Token public stakingToken;
    Token public rewardToken;
    HardStakingHandler public handler;

    function setUp() public {
        stakingToken = new Token();
        rewardToken = new Token();
        staking = new HardStaking(address(stakingToken), address(rewardToken));
        handler = new HardStakingHandler(staking, stakingToken, rewardToken);
        bytes4[] memory selectors = new bytes4[](5);
        selectors[0] = HardStakingHandler.handler_addRewards.selector;
        selectors[1] = HardStakingHandler.handler_stake.selector;
        selectors[2] = HardStakingHandler.handler_unstake.selector;
        selectors[3] = HardStakingHandler.handler_claimRewards.selector;
        selectors[4] = HardStakingHandler.handler_addNewActor.selector;
        targetSelector(FuzzSelector({addr: address(handler), selectors: selectors}));
        targetContract(address(handler));
    }

function invariant_earnedNeverExceedsAvailableRewards() external view {
    uint256 totalEarned = 0;
    uint256 len = handler.actorsLength();
    for (uint256 i = 0; i < len; i++) {
        address actor = handler.actors(i);
        totalEarned += staking.earned(actor);
    }
    uint256 available = rewardToken.balanceOf(address(staking));
    assertLe(
        totalEarned,
        available,
        "sum of earned() > reward token balance in contract"
    );
}

function invariant_ghostRewardClaimedNeverExceedsAdded() external view {
    assertLe(
        handler.ghost_rewards_claimed(),
        handler.ghost_rewards_added(),
        "ghost: actual tokens claimed > tokens added as rewards"
    );
}

function invariant_ghostTotalStaked() external view {
    assertEq(
        staking.totalStaked(),
        handler.ghost_total_staked(),
        "totalStaked != ghost_total_staked"
    );
}

function invariant_perActorStakingAccounting() external view {
    uint256 len = handler.actorsLength();
    for (uint256 i = 0; i < len; i++) {
        address actor = handler.actors(i);
        uint256 deposited = handler.ghost_staking_deposited(actor);
        uint256 withdrawn = handler.ghost_staking_withdrawn(actor);
        assertEq(
            staking.stakedOf(actor),
            deposited - withdrawn,
            "stakedOf actor != deposited - withdrawn"
        );
    }
}

function invariant_rewardClaimedNeverExceedsAdded() external view {
    assertLe(
        staking.totalRewardsClaimed(),
        staking.totalRewardsAdded(),
        "totalRewardsClaimed > totalRewardsAdded"
    );
}

function invariant_rewardTokenSolvency() external view {
    uint256 contractBalance = rewardToken.balanceOf(address(staking));
    uint256 added = handler.ghost_rewards_added();
    uint256 claimed = handler.ghost_rewards_claimed();
    assertGe(
        contractBalance,
        added - claimed,
        "reward token balance < (ghost_added - ghost_claimed)"
    );
}

function invariant_stakingTokenBalance() external view {
    assertEq(
        stakingToken.balanceOf(address(staking)),
        staking.totalStaked(),
        "staking token balance != totalStaked"
    );
}

function invariant_totalStakedConsistency() external view {
    uint256 sumStaked = 0;
    uint256 len = handler.actorsLength();
    for (uint256 i = 0; i < len; i++) {
        address actor = handler.actors(i);
        sumStaked += staking.stakedOf(actor);
    }
    assertEq(
        staking.totalStaked(),
        sumStaked,
        "totalStaked != sum of stakedOf actors"
    );
}

function invariant_userRewardPerTokenPaidNeverExceedsStored() external view {
    uint256 stored = staking.rewardPerTokenStored();
    uint256 len = handler.actorsLength();
    for (uint256 i = 0; i < len; i++) {
        address actor = handler.actors(i);
        assertLe(
            staking.userRewardPerTokenPaid(actor),
            stored,
            "userRewardPerTokenPaid > rewardPerTokenStored"
        );
    }
}

}