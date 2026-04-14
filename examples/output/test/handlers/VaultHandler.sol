// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {BaseHandler} from "../base/BaseHandler.sol";
import {Vault}       from "../../src/Vault.sol";
import {IERC20}      from "openzeppelin/token/ERC20/IERC20.sol";

contract VaultHandler is BaseHandler {

    Vault  public target;
    IERC20 public token;

    uint256 public ghost_totalDeposits    = 0;
    uint256 public ghost_totalWithdrawals = 0;

    constructor(Vault _target, IERC20 _token) {
        target = _target;
        token  = _token;
    }

    function handler_deposit(uint256 actorSeed, uint256 amount) external useActor(actorSeed) {
        amount = bound(amount, 1e6, target.depositCap() - target.totalAssets());
        vm.assume(token.balanceOf(_currentActor) >= amount);
        deal(address(token), _currentActor, amount);
        token.approve(address(target), amount);
        try target.deposit(amount) returns (uint256 shares) {
            calls[msg.sig]++;
            ghost_totalDeposits += amount;
        } catch {
            reverts[msg.sig]++;
        }
    }

    function handler_withdraw(uint256 actorSeed, uint256 amount) external useActor(actorSeed) {
        amount = bound(amount, 1, target.balanceOf(_currentActor));
        vm.assume(amount > 0);
        try target.withdraw(amount) {
            calls[msg.sig]++;
            ghost_totalWithdrawals += amount;
        } catch {
            reverts[msg.sig]++;
        }
    }

    function targetSelectors() public view override returns (FuzzSelector[] memory s) {
        s = new FuzzSelector[](7);
        s[0] = FuzzSelector({ addr: address(this), selectors: _arr(this.handler_deposit.selector) });
        s[1] = FuzzSelector({ addr: address(this), selectors: _arr(this.handler_deposit.selector) });
        s[2] = FuzzSelector({ addr: address(this), selectors: _arr(this.handler_deposit.selector) });
        s[3] = FuzzSelector({ addr: address(this), selectors: _arr(this.handler_deposit.selector) });
        s[4] = FuzzSelector({ addr: address(this), selectors: _arr(this.handler_withdraw.selector) });
        s[5] = FuzzSelector({ addr: address(this), selectors: _arr(this.handler_withdraw.selector) });
        s[6] = FuzzSelector({ addr: address(this), selectors: _arr(this.handler_withdraw.selector) });
    }

}