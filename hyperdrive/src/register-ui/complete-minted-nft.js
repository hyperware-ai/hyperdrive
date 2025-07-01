#!/usr/bin/env node

const { ethers } = require('ethers');
const fetch = require('node-fetch');
const crypto = require('crypto');
const argon2 = require('argon2');

// ABIs
const mechAbi = [
  {
    "inputs": [
      { "internalType": "address", "name": "to", "type": "address" },
      { "internalType": "uint256", "name": "value", "type": "uint256" },
      { "internalType": "bytes", "name": "data", "type": "bytes" },
      { "internalType": "uint8", "name": "operation", "type": "uint8" }
    ],
    "name": "execute",
    "outputs": [{ "internalType": "bytes", "name": "returnData", "type": "bytes" }],
    "stateMutability": "nonpayable",
    "type": "function"
  }
];

const multicallAbi = [
  {
    "inputs": [
      {
        "components": [
          { "internalType": "address", "name": "target", "type": "address" },
          { "internalType": "bytes", "name": "callData", "type": "bytes" }
        ],
        "internalType": "struct Multicall3.Call[]",
        "name": "calls",
        "type": "tuple[]"
      }
    ],
    "name": "aggregate",
    "outputs": [
      { "internalType": "uint256", "name": "blockNumber", "type": "uint256" },
      { "internalType": "bytes[]", "name": "returnData", "type": "bytes[]" }
    ],
    "stateMutability": "payable",
    "type": "function"
  }
];

const hypermapAbi = [
  {
    "inputs": [
      { "internalType": "bytes", "name": "label", "type": "bytes" },
      { "internalType": "bytes", "name": "data", "type": "bytes" }
    ],
    "name": "note",
    "outputs": [],
    "stateMutability": "nonpayable",
    "type": "function"
  }
];

// Contract addresses (update these for your network)
const HYPERMAP = '0x0000000000000000000000000000000000000000'; // Replace with actual address
const MULTICALL = '0xcA11bde05977b3631167028862bE2a173976CA11'; // Multicall3 address

// Helper functions
function stringToHex(str) {
  return '0x' + Buffer.from(str, 'utf8').toString('hex');
}

function portToBytes(port) {
  const buffer = Buffer.allocUnsafe(2);
  buffer.writeUInt16BE(port);
  return buffer;
}

function ipToBytes(ip) {
  const parts = ip.split('.').map(num => parseInt(num));
  return Buffer.from(parts);
}

function hyperhash(input) {
  return '0x' + crypto.createHash('sha256').update(input).digest('hex');
}

function encodeRouters(routers) {
  const hashedRouters = routers.map(router => hyperhash(router).slice(2));
  return '0x' + hashedRouters.join('');
}

async function generateNetworkingInfo() {
  try {
    const response = await fetch('http://localhost:8080/generate-networking-info', {
      method: 'POST'
    });
    return await response.json();
  } catch (error) {
    console.error('Failed to generate networking info:', error);
    throw error;
  }
}

async function completeMintedNFT({
  hnsName,
  tbaAddress,
  privateKey,
  providerUrl,
  direct = false,
  password
}) {
  // Create provider and wallet
  const provider = new ethers.JsonRpcProvider(providerUrl);
  const wallet = new ethers.Wallet(privateKey, provider);

  console.log(`Completing setup for ${hnsName} at TBA ${tbaAddress}...`);

  // Step 1: Generate networking info
  console.log('Generating networking keys...');
  const networkingInfo = await generateNetworkingInfo();

  const {
    networking_key,
    routing: {
      Both: {
        ip: ip_address,
        ports: { ws: ws_port, tcp: tcp_port },
        routers: allowed_routers
      }
    }
  } = networkingInfo;

  console.log('Networking key generated:', networking_key);

  // Step 2: Create multicall data
  const hypermapInterface = new ethers.Interface(hypermapAbi);

  const netkeycall = hypermapInterface.encodeFunctionData('note', [
    stringToHex('~net-key'),
    networking_key
  ]);

  const calls = [];
  calls.push({ target: HYPERMAP, callData: netkeycall });

  if (direct) {
    // Direct mode: store IP and ports
    const ws_port_call = hypermapInterface.encodeFunctionData('note', [
      stringToHex('~ws-port'),
      '0x' + portToBytes(ws_port || 0).toString('hex')
    ]);

    const tcp_port_call = hypermapInterface.encodeFunctionData('note', [
      stringToHex('~tcp-port'),
      '0x' + portToBytes(tcp_port || 0).toString('hex')
    ]);

    const ip_address_call = hypermapInterface.encodeFunctionData('note', [
      stringToHex('~ip'),
      '0x' + ipToBytes(ip_address).toString('hex')
    ]);

    calls.push({ target: HYPERMAP, callData: ws_port_call });
    calls.push({ target: HYPERMAP, callData: tcp_port_call });
    calls.push({ target: HYPERMAP, callData: ip_address_call });
  } else {
    // Indirect mode: store routers
    const encodedRouters = encodeRouters(allowed_routers);
    const router_call = hypermapInterface.encodeFunctionData('note', [
      stringToHex('~routers'),
      encodedRouters
    ]);

    calls.push({ target: HYPERMAP, callData: router_call });
  }

  // Step 3: Execute multicall from TBA
  const multicallInterface = new ethers.Interface(multicallAbi);
  const multicallData = multicallInterface.encodeFunctionData('aggregate', [calls]);

  const mechInterface = new ethers.Interface(mechAbi);
  const executeData = mechInterface.encodeFunctionData('execute', [
    MULTICALL,
    0n,
    multicallData,
    1
  ]);

  console.log('Sending transaction to store networking info...');
  const tba = new ethers.Contract(tbaAddress, mechAbi, wallet);
  const tx = await tba.execute(MULTICALL, 0n, multicallData, 1, {
    gasLimit: 500000n
  });

  console.log('Transaction sent:', tx.hash);
  const receipt = await tx.wait();
  console.log('Transaction confirmed!');

  // Step 4: Create boot data for password setting
  if (password) {
    console.log('Setting up password...');

    const minSaltL = 8;
    const nodeL = hnsName.length;
    const salt = nodeL >= minSaltL ? hnsName : hnsName.repeat(1 + Math.floor(minSaltL / nodeL));

    const hash = await argon2.hash(password, {
      salt: Buffer.from(salt),
      hashLength: 32,
      time: 2,
      memoryCost: 19456,
      type: argon2.argon2id
    });

    const hashedPasswordHex = '0x' + hash.toString('hex');
    const timestamp = Date.now();
    const chainId = (await provider.getNetwork()).chainId;

    // Create EIP-712 signature
    const domain = {
      name: "Hypermap",
      version: "1",
      chainId: chainId,
      verifyingContract: HYPERMAP
    };

    const types = {
      Boot: [
        { name: 'username', type: 'string' },
        { name: 'password_hash', type: 'bytes32' },
        { name: 'timestamp', type: 'uint256' },
        { name: 'direct', type: 'bool' },
        { name: 'reset', type: 'bool' },
        { name: 'chain_id', type: 'uint256' }
      ]
    };

    const message = {
      username: hnsName,
      password_hash: hashedPasswordHex,
      timestamp: BigInt(timestamp),
      direct: direct,
      reset: false,
      chain_id: BigInt(chainId)
    };

    const signature = await wallet.signTypedData(domain, types, message);

    // Send boot request
    const bootResponse = await fetch('http://localhost:8080/boot', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        password_hash: hashedPasswordHex,
        reset: false,
        username: hnsName,
        direct: direct,
        owner: wallet.address,
        timestamp: timestamp,
        signature: signature,
        chain_id: Number(chainId)
      })
    });

    if (bootResponse.ok) {
      const keyfile = await bootResponse.json();
      console.log('Password set successfully!');
      console.log('Keyfile (base64):', keyfile);
    } else {
      console.error('Failed to set password:', await bootResponse.text());
    }
  }

  console.log('\nSetup completed successfully!');
  console.log('HNS Name:', hnsName);
  console.log('TBA Address:', tbaAddress);
  console.log('Networking Key:', networking_key);
  console.log('Direct Mode:', direct);
  if (direct) {
    console.log('IP Address:', ip_address);
    console.log('WS Port:', ws_port);
    console.log('TCP Port:', tcp_port);
  } else {
    console.log('Routers:', allowed_routers);
  }
}

// CLI usage
if (require.main === module) {
  const args = process.argv.slice(2);

  if (args.length < 4) {
    console.log('Usage: node complete-minted-nft.js <hnsName> <tbaAddress> <privateKey> <providerUrl> [--direct] [--password <password>]');
    console.log('\nExample:');
    console.log('  node complete-minted-nft.js myname.os 0x123... 0xabc... https://eth-mainnet.g.alchemy.com/v2/... --password mypassword');
    process.exit(1);
  }

  const [hnsName, tbaAddress, privateKey, providerUrl] = args;
  const directIndex = args.indexOf('--direct');
  const passwordIndex = args.indexOf('--password');

  const config = {
    hnsName,
    tbaAddress,
    privateKey,
    providerUrl,
    direct: directIndex !== -1,
    password: passwordIndex !== -1 ? args[passwordIndex + 1] : undefined
  };

  completeMintedNFT(config).catch(console.error);
}

module.exports = { completeMintedNFT };
