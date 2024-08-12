import React, { useEffect, useRef } from "react";
import isValidDomain from "is-valid-domain";
import { toAscii } from "idna-uts46-hx";
import { usePublicClient } from 'wagmi'

import { KINOMAP, kinomapAbi } from '../abis'
import { kinohash } from "../utils/kinohash";

export const NAME_URL = "Name must contain only valid characters (a-z, 0-9, and -)";
export const NAME_LENGTH = "Name must be 9 characters or more";
export const NAME_CLAIMED = "Name is already claimed";
export const NAME_INVALID_PUNY = "Unsupported punycode character";
export const NAME_NOT_OWNER = "Name already exists and does not belong to this wallet";
export const NAME_NOT_REGISTERED = "Name is not registered";

type ClaimNameProps = {
  name: string;
  setName: React.Dispatch<React.SetStateAction<string>>;
  nameValidities: string[];
  setNameValidities: React.Dispatch<React.SetStateAction<string[]>>;
  triggerNameCheck: boolean;
  isReset?: boolean;
  topLevelZone?: string;
};

function EnterKnsName({
  name,
  setName,
  nameValidities,
  setNameValidities,
  triggerNameCheck,
  isReset = false,
  topLevelZone = ".os",
}: ClaimNameProps) {
  const client = usePublicClient();
  const debouncer = useRef<NodeJS.Timeout | null>(null);

  useEffect(() => {
    if (isReset) return;

    if (debouncer.current) clearTimeout(debouncer.current);

    debouncer.current = setTimeout(async () => {
      let index: number;
      let validities = [...nameValidities];

      const len = [...name].length;
      index = validities.indexOf(NAME_LENGTH);
      if (len < 9 && len !== 0) {
        if (index === -1) validities.push(NAME_LENGTH);
      } else if (index !== -1) validities.splice(index, 1);

      let normalized = ''
      index = validities.indexOf(NAME_INVALID_PUNY);
      try {
        normalized = toAscii(name + topLevelZone);
        if (index !== -1) validities.splice(index, 1);
      } catch (e) {
        if (index === -1) validities.push(NAME_INVALID_PUNY);
      }

      // only check if name is valid punycode
      if (normalized && normalized !== topLevelZone) {
        index = validities.indexOf(NAME_URL);
        if (name !== "" && !isValidDomain(normalized)) {
          if (index === -1) validities.push(NAME_URL);
        } else if (index !== -1) validities.splice(index, 1);

        index = validities.indexOf(NAME_CLAIMED);

        if (validities.length === 0 || index !== -1 && normalized.length > 2) {
          try {
            const namehash = kinohash(normalized)
            // maybe separate into helper function for readability?
            // also note picking the right chain ID & address!
            const data = await client?.readContract({
              address: KINOMAP,
              abi: kinomapAbi,
              functionName: "get",
              args: [namehash]
            })

            const owner = data?.[1];
            const owner_is_zero = owner === "0x0000000000000000000000000000000000000000";

            if (!owner_is_zero && index === -1) validities.push(NAME_CLAIMED);
          } catch (e) {
            console.error({ e })
            if (index !== -1) validities.splice(index, 1);
          }
        }
      }

      setNameValidities(validities);
    }, 100);
  }, [name, triggerNameCheck, isReset]);

  const noDots = (e: any) =>
    e.target.value.indexOf(".") === -1 && setName(e.target.value);

  return (
    <div className="enter-kns-name">
      <div className="input-wrapper">
        <input
          value={name}
          onChange={noDots}
          type="text"
          required
          name="name"
          placeholder="e.g. myname"
          className="kns-input"
        />
        <span className="kns-suffix">{topLevelZone}</span>
      </div>
      {nameValidities.map((x, i) => (
        <p key={i} className="error-message">{x}</p>
      ))}
    </div>
  );
}

export default EnterKnsName;
