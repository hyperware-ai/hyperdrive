import { multicallAbi, hypermapAbi, mechAbi, HYPERMAP, MULTICALL, HYPER_ACCOUNT_UPGRADABLE_IMPL } from "./";
import { encodeFunctionData, encodePacked, stringToHex } from "viem";

export function encodeMulticalls(metadataUri: string, metadataHash: string, tbaAddress?: `0x${string}`) {
    const metadataHashCall = encodeFunctionData({
        abi: hypermapAbi,
        functionName: 'note',
        args: [
            encodePacked(["bytes"], [stringToHex("~metadata-hash")]),
            encodePacked(["bytes"], [stringToHex(metadataHash)]),
        ]
    })

    const metadataUriCall = encodeFunctionData({
        abi: hypermapAbi,
        functionName: 'note',
        args: [
            encodePacked(["bytes"], [stringToHex("~metadata-uri")]),
            encodePacked(["bytes"], [stringToHex(metadataUri)]),
        ]
    })

    // Add initialize call if TBA address is provided
    const initializeCall = tbaAddress ? encodeFunctionData({
        abi: [{"inputs":[],"name":"initialize","outputs":[],"stateMutability":"nonpayable","type":"function"}],
        functionName: 'initialize',
        args: []
    }) : null;

    const baseCalls = [
        { target: HYPERMAP, callData: metadataHashCall },
        { target: HYPERMAP, callData: metadataUriCall },
    ];

    const calls = initializeCall && tbaAddress ?
        [{ target: tbaAddress, callData: initializeCall }, ...baseCalls] :
        baseCalls;

    const multicall = encodeFunctionData({
        abi: multicallAbi,
        functionName: 'aggregate',
        args: [calls]
    });
    return multicall;
}

export function encodeIntoMintCall(multicalls: `0x${string}`, our_address: `0x${string}`, app_name: string) {
    const initCall = encodeFunctionData({
        abi: mechAbi,
        functionName: 'execute',
        args: [
            MULTICALL,
            BigInt(0),
            multicalls,
            1
        ]
    });

    const mintCall = encodeFunctionData({
        abi: hypermapAbi,
        functionName: 'mint',
        args: [
            our_address,
            encodePacked(["bytes"], [stringToHex(app_name)]),
            initCall,
            HYPER_ACCOUNT_UPGRADABLE_IMPL,
        ]
    })
    return mintCall;
}
