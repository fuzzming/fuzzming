pragma solidity ^0.8.20;
import "forge-std/Test.sol";

contract VaultHandler {
    Vault public target;
    address[] public actors;

    constructor(Vault _target) {
        target = _target;
    }

    function deposit(address caller, uint256 amount) external {
        actors.push(caller);
        payable(address(target)).transfer(amount);
        target.deposit{value: amount}();
    }

    function withdraw(address caller, uint256 amount) external {
        actors.push(caller);
        target.withdraw(amount);
    }
}