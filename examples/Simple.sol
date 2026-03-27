pragma solidity ^0.8.0;

contract Simple {
    uint256 public total;
    address owner; // owner address

    modifier onlyOwner() {
        require(msg.sender == owner);
        _;
    }

    function deposit(uint256 amount) public {
        total += amount;
    }

    function withdraw(uint256 amount) public {
        if (amount == 0) { return; }
        require(total >= amount);
        total -= amount;
    }
}
