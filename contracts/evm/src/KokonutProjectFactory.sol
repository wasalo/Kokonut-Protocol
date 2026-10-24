// SPDX-License-Identifier: MIT
pragma solidity 0.8.38;

interface IInitializableProjectComponent {
    function initialize(bytes32 projectId_, address projectAdmin_) external;
    function projectId() external view returns (bytes32);
    function projectAdmin() external view returns (address);
}

/// @notice Testnet-development factory for pre-approved project component creation code.
/// @dev This non-custodial prototype is not audited or approved for production use.
contract KokonutProjectFactory {
    error Unauthorized();
    error InvalidConfigurationAuthority();
    error InvalidDeploymentAuthority();
    error InvalidAuthoritySeparation();
    error InvalidCreationCodeHash();
    error InvalidProjectId();
    error InvalidProjectAdmin();
    error CreationCodeNotAllowed();
    error ProjectAlreadyDeployed();
    error Create2DeploymentFailed();
    error ProjectInitializationMismatch();

    address public immutable configurationAuthority;
    address public immutable deploymentAuthority;

    mapping(bytes32 creationCodeHash => bool allowed) public allowedCreationCodeHashes;
    mapping(bytes32 projectId => address component) public projectById;

    event CreationCodeHashPermissionChanged(bytes32 indexed creationCodeHash, bool allowed);
    event ProjectComponentDeployed(
        bytes32 indexed projectId, address indexed component, address indexed projectAdmin, bytes32 creationCodeHash
    );

    constructor(address configurationAuthority_, address deploymentAuthority_) {
        if (configurationAuthority_ == address(0)) revert InvalidConfigurationAuthority();
        if (deploymentAuthority_ == address(0)) revert InvalidDeploymentAuthority();
        if (configurationAuthority_ == deploymentAuthority_) revert InvalidAuthoritySeparation();
        configurationAuthority = configurationAuthority_;
        deploymentAuthority = deploymentAuthority_;
    }

    function setCreationCodeHashAllowed(bytes32 creationCodeHash, bool allowed) external {
        if (msg.sender != configurationAuthority) revert Unauthorized();
        if (creationCodeHash == bytes32(0)) revert InvalidCreationCodeHash();
        allowedCreationCodeHashes[creationCodeHash] = allowed;
        emit CreationCodeHashPermissionChanged(creationCodeHash, allowed);
    }

    function deployProject(bytes32 projectId, address projectAdmin, bytes calldata creationCode)
        external
        returns (address component)
    {
        if (msg.sender != deploymentAuthority) revert Unauthorized();
        if (projectId == bytes32(0)) revert InvalidProjectId();
        if (
            projectAdmin == address(0) || projectAdmin.code.length == 0 || projectAdmin == address(this)
                || projectAdmin == configurationAuthority || projectAdmin == deploymentAuthority
        ) {
            revert InvalidProjectAdmin();
        }
        if (projectById[projectId] != address(0)) revert ProjectAlreadyDeployed();

        bytes32 creationCodeHash = keccak256(creationCode);
        if (!allowedCreationCodeHashes[creationCodeHash]) revert CreationCodeNotAllowed();

        bytes32 salt = _projectSalt(projectId);
        address predicted = _predictProjectAddress(projectId, creationCodeHash);

        // Reserve and announce before calling approved creation code. If creation
        // or initialization fails, EVM transaction rollback removes both effects.
        projectById[projectId] = predicted;
        emit ProjectComponentDeployed(projectId, predicted, projectAdmin, creationCodeHash);

        bytes memory initCode = creationCode;
        // CREATE2 is required to keep project addresses deterministic.
        // slither-disable-next-line assembly
        assembly ("memory-safe") {
            component := create2(0, add(initCode, 0x20), mload(initCode), salt)
        }
        if (component == address(0) || component.code.length == 0 || component != predicted) {
            revert Create2DeploymentFailed();
        }

        try IInitializableProjectComponent(component).initialize(projectId, projectAdmin) {
            if (
                IInitializableProjectComponent(component).projectId() != projectId
                    || IInitializableProjectComponent(component).projectAdmin() != projectAdmin
            ) {
                revert ProjectInitializationMismatch();
            }
        } catch {
            revert ProjectInitializationMismatch();
        }
    }

    function predictProjectAddress(bytes32 projectId, bytes32 creationCodeHash) external view returns (address) {
        if (projectId == bytes32(0)) revert InvalidProjectId();
        if (!allowedCreationCodeHashes[creationCodeHash]) revert CreationCodeNotAllowed();
        return _predictProjectAddress(projectId, creationCodeHash);
    }

    function _predictProjectAddress(bytes32 projectId, bytes32 creationCodeHash) private view returns (address) {
        bytes32 digest =
            keccak256(abi.encodePacked(bytes1(0xff), address(this), _projectSalt(projectId), creationCodeHash));
        return address(uint160(uint256(digest)));
    }

    function _projectSalt(bytes32 projectId) private pure returns (bytes32) {
        return keccak256(abi.encode("KOKONUT_PROJECT_COMPONENT_V1", projectId));
    }
}
