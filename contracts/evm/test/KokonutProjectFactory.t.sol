// SPDX-License-Identifier: MIT
pragma solidity 0.8.38;

import {KokonutProjectFactory} from "../src/KokonutProjectFactory.sol";

interface Vm {
    function expectRevert(bytes4 selector) external;
    function prank(address caller) external;
    function assume(bool condition) external;
}

contract MockProjectComponent {
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
}

contract MockMismatchingProjectComponent {
    bytes32 public projectId;
    address public projectAdmin;

    function initialize(bytes32 projectId_, address projectAdmin_) external {
        projectId = projectId_ ^ bytes32(uint256(1));
        projectAdmin = projectAdmin_;
    }
}

contract MockProjectAdmin {}

contract KokonutProjectFactoryTests {
    Vm private constant vm = Vm(address(uint160(uint256(keccak256("hevm cheat code")))));
    address private constant CONFIG_AUTHORITY = address(0xA11CE);
    address private constant DEPLOYMENT_AUTHORITY = address(0xB0B);
    bytes32 private constant PROJECT_ID = keccak256("adelphi-project");
    address private immutable PROJECT_ADMIN;

    constructor() {
        PROJECT_ADMIN = address(new MockProjectAdmin());
    }

    function testDeploysAllowlistedComponentAndInitializesProjectAuthority() public {
        bytes memory initCode = type(MockProjectComponent).creationCode;
        KokonutProjectFactory factory = _newFactory(keccak256(initCode));
        vm.prank(DEPLOYMENT_AUTHORITY);
        address deployed = factory.deployProject(PROJECT_ID, PROJECT_ADMIN, initCode);
        MockProjectComponent component = MockProjectComponent(deployed);
        require(component.projectId() == PROJECT_ID, "project ID not initialized");
        require(component.projectAdmin() == PROJECT_ADMIN, "admin not initialized");
        require(factory.projectById(PROJECT_ID) == deployed, "project not registered");
    }

    function testOnlyConfiguredDeploymentAuthorityCanDeploy() public {
        bytes memory initCode = type(MockProjectComponent).creationCode;
        KokonutProjectFactory factory = _newFactory(keccak256(initCode));
        vm.expectRevert(KokonutProjectFactory.Unauthorized.selector);
        factory.deployProject(PROJECT_ID, PROJECT_ADMIN, initCode);
    }

    function testRejectsUnapprovedCreationCodeHash() public {
        bytes memory initCode = type(MockProjectComponent).creationCode;
        KokonutProjectFactory factory = _newFactory(bytes32(uint256(1)));
        vm.prank(DEPLOYMENT_AUTHORITY);
        vm.expectRevert(KokonutProjectFactory.CreationCodeNotAllowed.selector);
        factory.deployProject(PROJECT_ID, PROJECT_ADMIN, initCode);
    }

    function testConfigurationAuthorityCanAllowAndRevokeCreationCodeHash() public {
        bytes memory initCode = type(MockProjectComponent).creationCode;
        bytes32 codeHash = keccak256(initCode);
        KokonutProjectFactory factory = _newFactory(bytes32(uint256(1)));
        vm.prank(CONFIG_AUTHORITY);
        factory.setCreationCodeHashAllowed(codeHash, true);
        vm.prank(DEPLOYMENT_AUTHORITY);
        factory.deployProject(PROJECT_ID, PROJECT_ADMIN, initCode);
        require(factory.projectById(PROJECT_ID) != address(0), "project not deployed");

        vm.prank(CONFIG_AUTHORITY);
        factory.setCreationCodeHashAllowed(codeHash, false);
        require(!factory.allowedCreationCodeHashes(codeHash), "code hash still allowed");

        vm.prank(DEPLOYMENT_AUTHORITY);
        vm.expectRevert(KokonutProjectFactory.CreationCodeNotAllowed.selector);
        factory.deployProject(keccak256("another-project"), PROJECT_ADMIN, initCode);
    }

    function testOnlyConfigurationAuthorityCanChangeAllowlist() public {
        bytes memory initCode = type(MockProjectComponent).creationCode;
        KokonutProjectFactory factory = _newFactory(keccak256(initCode));
        vm.expectRevert(KokonutProjectFactory.Unauthorized.selector);
        factory.setCreationCodeHashAllowed(bytes32(uint256(2)), true);
    }

    function testRejectsZeroProjectIdAndZeroProjectAdmin() public {
        bytes memory initCode = type(MockProjectComponent).creationCode;
        KokonutProjectFactory factory = _newFactory(keccak256(initCode));
        vm.prank(DEPLOYMENT_AUTHORITY);
        vm.expectRevert(KokonutProjectFactory.InvalidProjectId.selector);
        factory.deployProject(bytes32(0), PROJECT_ADMIN, initCode);

        vm.prank(DEPLOYMENT_AUTHORITY);
        vm.expectRevert(KokonutProjectFactory.InvalidProjectAdmin.selector);
        factory.deployProject(PROJECT_ID, address(0), initCode);
    }

    function testProjectIdCannotBeDeployedTwice() public {
        bytes memory initCode = type(MockProjectComponent).creationCode;
        KokonutProjectFactory factory = _newFactory(keccak256(initCode));
        vm.prank(DEPLOYMENT_AUTHORITY);
        factory.deployProject(PROJECT_ID, PROJECT_ADMIN, initCode);
        vm.prank(DEPLOYMENT_AUTHORITY);
        vm.expectRevert(KokonutProjectFactory.ProjectAlreadyDeployed.selector);
        factory.deployProject(PROJECT_ID, PROJECT_ADMIN, initCode);
    }

    function testPredictedAddressMatchesCreate2Deployment() public {
        bytes memory initCode = type(MockProjectComponent).creationCode;
        KokonutProjectFactory factory = _newFactory(keccak256(initCode));
        address predicted = factory.predictProjectAddress(PROJECT_ID, keccak256(initCode));
        vm.prank(DEPLOYMENT_AUTHORITY);
        address deployed = factory.deployProject(PROJECT_ID, PROJECT_ADMIN, initCode);
        require(deployed == predicted, "CREATE2 address mismatch");
    }

    function testWrongInitializationRollsBackDeploymentAndRegistration() public {
        bytes memory initCode = type(MockMismatchingProjectComponent).creationCode;
        KokonutProjectFactory factory = _newFactory(keccak256(initCode));
        vm.prank(DEPLOYMENT_AUTHORITY);
        vm.expectRevert(KokonutProjectFactory.ProjectInitializationMismatch.selector);
        factory.deployProject(PROJECT_ID, PROJECT_ADMIN, initCode);
        require(factory.projectById(PROJECT_ID) == address(0), "failed deployment was retained");
    }

    function testProjectAdminMustBeAContract() public {
        bytes memory initCode = type(MockProjectComponent).creationCode;
        KokonutProjectFactory factory = _newFactory(keccak256(initCode));
        vm.prank(DEPLOYMENT_AUTHORITY);
        vm.expectRevert(KokonutProjectFactory.InvalidProjectAdmin.selector);
        factory.deployProject(PROJECT_ID, address(0xCAFE), initCode);
    }

    function testConfigurationAuthorityCannotRemainProjectAdmin() public {
        address configurationAuthority = address(new MockProjectAdmin());
        address deploymentAuthority = address(new MockProjectAdmin());
        bytes memory initCode = type(MockProjectComponent).creationCode;
        KokonutProjectFactory factory = new KokonutProjectFactory(configurationAuthority, deploymentAuthority);
        vm.prank(configurationAuthority);
        factory.setCreationCodeHashAllowed(keccak256(initCode), true);

        vm.prank(deploymentAuthority);
        vm.expectRevert(KokonutProjectFactory.InvalidProjectAdmin.selector);
        factory.deployProject(PROJECT_ID, configurationAuthority, initCode);
    }

    function testDeploymentAuthorityCannotRemainProjectAdmin() public {
        address configurationAuthority = address(new MockProjectAdmin());
        address deploymentAuthority = address(new MockProjectAdmin());
        bytes memory initCode = type(MockProjectComponent).creationCode;
        KokonutProjectFactory factory = new KokonutProjectFactory(configurationAuthority, deploymentAuthority);
        vm.prank(configurationAuthority);
        factory.setCreationCodeHashAllowed(keccak256(initCode), true);

        vm.prank(deploymentAuthority);
        vm.expectRevert(KokonutProjectFactory.InvalidProjectAdmin.selector);
        factory.deployProject(PROJECT_ID, deploymentAuthority, initCode);
    }

    function testFactoryCannotRemainProjectAdmin() public {
        address configurationAuthority = address(new MockProjectAdmin());
        address deploymentAuthority = address(new MockProjectAdmin());
        bytes memory initCode = type(MockProjectComponent).creationCode;
        KokonutProjectFactory factory = new KokonutProjectFactory(configurationAuthority, deploymentAuthority);
        vm.prank(configurationAuthority);
        factory.setCreationCodeHashAllowed(keccak256(initCode), true);

        vm.prank(deploymentAuthority);
        vm.expectRevert(KokonutProjectFactory.InvalidProjectAdmin.selector);
        factory.deployProject(PROJECT_ID, address(factory), initCode);
    }

    function testFuzzDeploymentMatchesPredictedAddress(bytes32 projectId) public {
        vm.assume(projectId != bytes32(0));
        bytes memory initCode = type(MockProjectComponent).creationCode;
        bytes32 codeHash = keccak256(initCode);
        KokonutProjectFactory factory = _newFactory(codeHash);
        address predicted = factory.predictProjectAddress(projectId, codeHash);

        vm.prank(DEPLOYMENT_AUTHORITY);
        address deployed = factory.deployProject(projectId, PROJECT_ADMIN, initCode);

        require(deployed == predicted, "CREATE2 address mismatch");
        require(factory.projectById(projectId) == predicted, "registered address mismatch");
    }

    function testFuzzDistinctProjectIdsHaveDistinctPredictedAddresses(bytes32 projectId, bytes32 otherProjectId)
        public
    {
        vm.assume(projectId != bytes32(0));
        vm.assume(otherProjectId != bytes32(0));
        vm.assume(projectId != otherProjectId);
        bytes memory initCode = type(MockProjectComponent).creationCode;
        bytes32 codeHash = keccak256(initCode);
        KokonutProjectFactory factory = _newFactory(codeHash);

        address predicted = factory.predictProjectAddress(projectId, codeHash);
        address otherPredicted = factory.predictProjectAddress(otherProjectId, codeHash);

        require(predicted != otherPredicted, "distinct IDs collided");
    }

    function testFuzzFailedInitializationRollsBackAddressAndRegistration(bytes32 projectId) public {
        vm.assume(projectId != bytes32(0));
        bytes memory initCode = type(MockMismatchingProjectComponent).creationCode;
        bytes32 codeHash = keccak256(initCode);
        KokonutProjectFactory factory = _newFactory(codeHash);
        address predicted = factory.predictProjectAddress(projectId, codeHash);

        vm.prank(DEPLOYMENT_AUTHORITY);
        vm.expectRevert(KokonutProjectFactory.ProjectInitializationMismatch.selector);
        factory.deployProject(projectId, PROJECT_ADMIN, initCode);

        require(factory.projectById(projectId) == address(0), "failed deployment was registered");
        require(predicted.code.length == 0, "failed deployment code was retained");
    }

    function testConfigurationAndDeploymentAuthoritiesMustBeDistinct() public {
        vm.expectRevert(KokonutProjectFactory.InvalidAuthoritySeparation.selector);
        new KokonutProjectFactory(CONFIG_AUTHORITY, CONFIG_AUTHORITY);
    }

    function _newFactory(bytes32 initialAllowedHash) private returns (KokonutProjectFactory) {
        KokonutProjectFactory factory = new KokonutProjectFactory(CONFIG_AUTHORITY, DEPLOYMENT_AUTHORITY);
        vm.prank(CONFIG_AUTHORITY);
        factory.setCreationCodeHashAllowed(initialAllowedHash, true);
        return factory;
    }
}
