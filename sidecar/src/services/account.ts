import { ApiPromise } from "@polkadot/api";
import { AccountResponse } from "../types";
import { fromBech32 } from "@cosmjs/encoding";
import { Codec } from "@polkadot/types/types/index.js";
import { stringToU8a, u8aConcat, u8aToHex } from "@polkadot/util";
import { blake2AsU8a } from "@polkadot/util-crypto";
import { ApiService } from "./service";
import Dummy from "../constants/dummy";
import { AccountInfo } from "@polkadot/types/interfaces";

export class AccountService implements ApiService {
	chainApi: ApiPromise;

	constructor(chainApi: ApiPromise) {
		this.chainApi = chainApi;
	}

	public async accounts(address: string, blockHash?: string): Promise<AccountResponse> {
		console.debug(`accounts(${address}, ${blockHash})`);

		let sequence = '0';
		let origin = (await this.origin(address)).toString();
		if (!origin) {
			origin = this.interim(address);
		}
		const account = await (await (blockHash ? this.chainApi.at(blockHash) : this.chainApi)).query['system']['account'](origin);

		if (account) {
			const { nonce } = account.toJSON() as unknown as AccountInfo;
			sequence = nonce.toString();
		}
		return new AccountResponse({
			'@type': '/cosmos.auth.v1beta1.BaseAccount',
			address,
			pub_key: {
				'@type': '/cosmos.crypto.secp256k1.PubKey',
				key: Dummy.Secp256k1PublicKey,
			},
			account_number: '0',
			sequence,
		});
	}

	public async origin(address: string): Promise<Codec> {
		const { data } = fromBech32(address);
		return this.chainApi.query['addressMap']['index'](
			Buffer.concat([Buffer.from([0x00]), data])
		);
	}

	public interim(address: string): string {
		const { data } = fromBech32(address);
		const addressRaw = blake2AsU8a(u8aConcat(stringToU8a('cosm:'), data));
		return u8aToHex(addressRaw);
	}
}
