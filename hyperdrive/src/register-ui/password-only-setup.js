#!/usr/bin/env node

const { ethers } = require('ethers');
const fetch = require('node-fetch');
const argon2 = require('argon2');
const fs = require('fs').promises;
const path = require('path');

// Contract address
const HYPERMAP = '0x0000000000000000000000000000000000000000'; // Replace with actual address

async function passwordOnlySetup({
  hnsName,
  password,
  privateKey,
  providerUrl,
  direct = false,
  chainId,
  outputFile
}) {
  console.log(`Setting up password for ${hnsName}...`);

  // Create provider and wallet
  const provider = new ethers.JsonRpcProvider(providerUrl);
  const wallet = new ethers.Wallet(privateKey, provider);

  // Get chain ID if not provided
  if (!chainId) {
    const network = await provider.getNetwork();
    chainId = Number(network.chainId);
  }

  // Hash the password
  const minSaltL = 8;
  const nodeL = hnsName.length;
  const salt = nodeL >= minSaltL ? hnsName : hnsName.repeat(1 + Math.floor(minSaltL / nodeL));

  console.log('Hashing password...');
  const hash = await argon2.hash(password, {
    salt: Buffer.from(salt),
    hashLength: 32,
    time: 2,
    memoryCost: 19456,
    type: argon2.argon2id
  });

  const hashedPasswordHex = '0x' + hash.toString('hex');
  const timestamp = Date.now();

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

  console.log('Signing message...');
  const signature = await wallet.signTypedData(domain, types, message);

  // Send boot request
  console.log('Sending boot request...');
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
      chain_id: chainId
    })
  });

  if (bootResponse.ok) {
    const keyfileBase64 = await bootResponse.json();
    console.log('Password set successfully!');

    // Save keyfile if output path provided
    if (outputFile) {
      const keyfileContent = Buffer.from(keyfileBase64, 'base64');
      await fs.writeFile(outputFile, keyfileContent);
      console.log(`Keyfile saved to: ${outputFile}`);
    } else {
      console.log('Keyfile (base64):');
      console.log(keyfileBase64);
    }

    console.log('\n✅ Setup completed successfully!');
    console.log('HNS Name:', hnsName);
    console.log('Owner:', wallet.address);
    console.log('Direct Mode:', direct);
    console.log('Chain ID:', chainId);
  } else {
    const errorText = await bootResponse.text();
    console.error('Failed to set password:', errorText);
    process.exit(1);
  }
}

// CLI usage
if (require.main === module) {
  const args = process.argv.slice(2);

  if (args.length < 4) {
    console.log('Usage: node password-only-setup.js <hnsName> <password> <privateKey> <providerUrl> [options]');
    console.log('\nOptions:');
    console.log('  --direct              Set if this is a direct node');
    console.log('  --chain-id <id>       Specify chain ID (auto-detected if not provided)');
    console.log('  --output <path>       Save keyfile to specified path');
    console.log('\nExample:');
    console.log('  node password-only-setup.js myname.os mypassword 0xabc... https://eth-mainnet.g.alchemy.com/v2/... --output myname.keyfile');
    process.exit(1);
  }

  const [hnsName, password, privateKey, providerUrl] = args;
  const directIndex = args.indexOf('--direct');
  const chainIdIndex = args.indexOf('--chain-id');
  const outputIndex = args.indexOf('--output');

  const config = {
    hnsName,
    password,
    privateKey,
    providerUrl,
    direct: directIndex !== -1,
    chainId: chainIdIndex !== -1 ? parseInt(args[chainIdIndex + 1]) : undefined,
    outputFile: outputIndex !== -1 ? args[outputIndex + 1] : undefined
  };

  passwordOnlySetup(config).catch(error => {
    console.error('Error:', error);
    process.exit(1);
  });
}

module.exports = { passwordOnlySetup };
