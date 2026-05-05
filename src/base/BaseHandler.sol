// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import {Test} from "forge-std/Test.sol";

abstract contract BaseHandler is Test {
    address[] internal _actors;
    mapping(address => bool) internal _isActor;

    function addActor(address actor) public {
        if (_isActor[actor]) return;
        _isActor[actor] = true;
        _actors.push(actor);
    }

    function addActors(address[] memory actors) public {
        for (uint256 i = 0; i < actors.length; i++) {
            addActor(actors[i]);
        }
    }

    function actors() public view returns (address[] memory) {
        return _actors;
    }

    function actorCount() public view returns (uint256) {
        return _actors.length;
    }

    function actorAt(uint256 idx) public view returns (address) {
        return _actors[idx];
    }

    // Optional: allow handlers to control which functions are fuzzed in setUp().
    function targetSelectors(bytes4[] memory selectors) internal {
        targetSelector(FuzzSelector({addr: address(this), selectors: selectors}));
    }
}
