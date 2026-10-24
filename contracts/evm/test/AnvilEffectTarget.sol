// SPDX-License-Identifier: MIT
pragma solidity 0.8.38;

/// @dev Ephemeral local-Anvil fixture. Never used as a production component.
contract AnvilEffectTarget {
    event ValueSet(bytes32 indexed requestId, uint256 value);

    uint256 public value;

    function setValue(bytes32 requestId, uint256 nextValue) external {
        value = nextValue;
        emit ValueSet(requestId, nextValue);
    }
}
