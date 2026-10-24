// SPDX-License-Identifier: MIT
pragma solidity 0.8.38;

import {KokonutProjectFactory, IInitializableProjectComponent} from "../src/KokonutProjectFactory.sol";

interface InvariantVm {
    function prank(address caller) external;
}

contract InvariantMockProjectAdmin {}

contract InvariantMockProjectComponent {
    bytes32 public projectId;
    address public projectAdmin;
    bool private initialized;

    error AlreadyInitialized();

    function initialize(bytes32 projectId_, address projectAdmin_) external {
        if (initialized) revert AlreadyInitialized();
        initialized = true;
        projectId = projectId_;
        projectAdmin = projectAdmin_;
    }

    function authoritySurfaces() external view returns (bytes32[] memory roles, address[] memory controllers) {
        bytes32 projectAdminRole = keccak256("KOKONUT_PROJECT_ADMIN");
        bytes32 upgradeRole = keccak256("KOKONUT_UPGRADE_AUTHORITY");
        roles = new bytes32[](2);
        controllers = new address[](2);
        if (projectAdminRole < upgradeRole) {
            roles[0] = projectAdminRole;
            controllers[0] = projectAdmin;
            roles[1] = upgradeRole;
        } else {
            roles[0] = upgradeRole;
            roles[1] = projectAdminRole;
            controllers[1] = projectAdmin;
        }
    }
}

contract KokonutProjectFactoryHandler {
    InvariantVm private constant vm = InvariantVm(address(uint160(uint256(keccak256("hevm cheat code")))));
    uint256 private constant MAX_PROJECTS = 64;
    address private constant DEPLOYMENT_AUTHORITY = address(0xB0B);

    KokonutProjectFactory public immutable factory;
    address public immutable projectAdmin;
    bytes32 public immutable creationCodeHash;
    bytes private creationCode;
    bytes32[] private _projectIds;
    address[] private _components;

    constructor(KokonutProjectFactory factory_, address projectAdmin_, bytes memory creationCode_) {
        factory = factory_;
        projectAdmin = projectAdmin_;
        creationCode = creationCode_;
        creationCodeHash = keccak256(creationCode_);
    }

    function deployOne() external {
        _deployOne();
    }

    function deployBatch(uint8 requestedCount) external {
        uint256 count = uint256(requestedCount) % 4 + 1;
        for (uint256 i; i < count; ++i) {
            _deployOne();
        }
    }

    function deploymentCount() external view returns (uint256) {
        return _projectIds.length;
    }

    function projectIdAt(uint256 index) external view returns (bytes32) {
        if (index >= _projectIds.length) return bytes32(0);
        return _projectIds[index];
    }

    function componentAt(uint256 index) external view returns (address) {
        if (index >= _components.length) return address(0);
        return _components[index];
    }

    function _deployOne() private {
        if (_projectIds.length >= MAX_PROJECTS) return;
        bytes32 projectId = bytes32(_projectIds.length + 1);
        vm.prank(DEPLOYMENT_AUTHORITY);
        address component = factory.deployProject(projectId, projectAdmin, creationCode);
        _projectIds.push(projectId);
        _components.push(component);
    }
}

/// @dev Minimal target registration surface; the fuzz campaign calls only the stateful handler.
contract KokonutProjectFactoryInvariantTests {
    InvariantVm private constant vm = InvariantVm(address(uint160(uint256(keccak256("hevm cheat code")))));
    address private constant CONFIGURATION_AUTHORITY = address(0xA11CE);
    address private constant DEPLOYMENT_AUTHORITY = address(0xB0B);
    uint256 private constant MAX_PROJECTS = 64;

    KokonutProjectFactory private factory;
    KokonutProjectFactoryHandler private handler;
    address private projectAdmin;
    bytes32 private creationCodeHash;
    address[] private _targetedContracts;

    function setUp() public {
        projectAdmin = address(new InvariantMockProjectAdmin());
        factory = new KokonutProjectFactory(CONFIGURATION_AUTHORITY, DEPLOYMENT_AUTHORITY);
        bytes memory creationCode = type(InvariantMockProjectComponent).creationCode;
        creationCodeHash = keccak256(creationCode);
        vm.prank(CONFIGURATION_AUTHORITY);
        factory.setCreationCodeHashAllowed(creationCodeHash, true);
        handler = new KokonutProjectFactoryHandler(factory, projectAdmin, creationCode);
        _targetedContracts.push(address(handler));
    }

    function targetContracts() external view returns (address[] memory) {
        return _targetedContracts;
    }

    function invariant_registeredProjectsRemainUniqueAuthorizedAndPredictable() public view {
        uint256 count = handler.deploymentCount();
        require(count <= MAX_PROJECTS, "handler exceeded project bound");
        require(handler.projectIdAt(count) == bytes32(0), "unexpected ghost project after history");

        for (uint256 i; i < count; ++i) {
            bytes32 projectId = handler.projectIdAt(i);
            address componentAddress = handler.componentAt(i);
            require(projectId == bytes32(i + 1), "project IDs are not unique and sequential");
            require(componentAddress.code.length != 0, "registered component has no code");
            require(factory.projectById(projectId) == componentAddress, "registry differs from handler history");

            bytes32 salt = keccak256(abi.encode("KOKONUT_PROJECT_COMPONENT_V1", projectId));
            address predicted = address(
                uint160(uint256(keccak256(abi.encodePacked(bytes1(0xff), address(factory), salt, creationCodeHash))))
            );
            require(componentAddress == predicted, "component address differs from independent CREATE2 prediction");

            IInitializableProjectComponent component = IInitializableProjectComponent(componentAddress);
            require(component.projectId() == projectId, "component project ID mismatch");
            require(component.projectAdmin() == projectAdmin, "component project admin mismatch");
            (bytes32[] memory roles, address[] memory controllers) = component.authoritySurfaces();
            require(roles.length != 0 && roles.length <= 32, "manifest count outside accepted range");
            require(controllers.length == roles.length, "manifest arrays differ in length");
            bool projectAdminSurfaceFound;
            for (uint256 j; j < roles.length; ++j) {
                require(roles[j] != bytes32(0), "zero role in manifest");
                if (j != 0) require(roles[j] > roles[j - 1], "manifest roles not strictly ordered");
                require(controllers[j] == address(0) || controllers[j] == projectAdmin, "split authority in manifest");
                if (roles[j] == factory.PROJECT_ADMIN_ROLE()) {
                    require(controllers[j] == projectAdmin, "root admin surface mismatched");
                    projectAdminSurfaceFound = true;
                }
            }
            require(projectAdminSurfaceFound, "root admin surface missing");

            for (uint256 prior; prior < i; ++prior) {
                require(handler.componentAt(prior) != componentAddress, "two project IDs share a component address");
            }
        }
    }
}
