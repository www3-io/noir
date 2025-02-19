// This file is part of Noir.

// Copyright (C) Haderech Pte. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

#![cfg_attr(not(feature = "std"), no_std)]

pub mod gas;

use cosmos_sdk_proto::{cosmos::bank::v1beta1::MsgSend, prost::Message, Any};
use frame_support::{
	ensure,
	traits::{
		fungibles::Mutate,
		tokens::{currency::Currency, Preservation},
		ExistenceRequirement,
	},
};
use gas::GasInfo;
use nostd::{marker::PhantomData, vec};
use pallet_cosmos::AddressMapping;
use pallet_cosmos_types::{
	address::{acc_address_from_bech32, AUTH_ADDRESS_LEN},
	coin::traits::Coins,
	context,
	errors::{CosmosError, RootError},
	events::{
		traits::EventManager, CosmosEvent, EventAttribute, ATTRIBUTE_KEY_AMOUNT,
		ATTRIBUTE_KEY_SENDER,
	},
	gas::traits::GasMeter,
	msgservice::traits::MsgHandler,
};
use pallet_cosmos_x_bank_types::events::{ATTRIBUTE_KEY_RECIPIENT, EVENT_TYPE_TRANSFER};
use sp_core::{Get, H160};
use sp_runtime::{
	traits::{TryConvertBack, Zero},
	SaturatedConversion,
};

pub struct MsgSendHandler<T>(PhantomData<T>);

impl<T> Default for MsgSendHandler<T> {
	fn default() -> Self {
		Self(Default::default())
	}
}

impl<T, Context> MsgHandler<Context> for MsgSendHandler<T>
where
	T: pallet_cosmos::Config,
	Context: context::traits::Context,
{
	fn handle(&self, ctx: &mut Context, msg: &Any) -> Result<(), CosmosError> {
		let MsgSend { from_address, to_address, amount } =
			MsgSend::decode(&mut &*msg.value).map_err(|_| RootError::UnpackAnyError)?;

		let (_hrp, from_address_raw) =
			acc_address_from_bech32(&from_address).map_err(|_| RootError::InvalidAddress)?;
		ensure!(from_address_raw.len() == AUTH_ADDRESS_LEN, RootError::InvalidAddress);
		let from_account = T::AddressMapping::into_account_id(H160::from_slice(&from_address_raw));

		let (_hrp, to_address_raw) =
			acc_address_from_bech32(&to_address).map_err(|_| RootError::InvalidAddress)?;
		ensure!(to_address_raw.len() == AUTH_ADDRESS_LEN, RootError::InvalidAddress);
		let to_account = T::AddressMapping::into_account_id(H160::from_slice(&to_address_raw));

		for amt in amount.iter() {
			let amount = amt.amount.parse::<u128>().map_err(|_| RootError::InvalidCoins)?;

			if T::NativeDenom::get() == amt.denom {
				ctx.gas_meter()
					.consume_gas(GasInfo::<T>::msg_send_native(), "msg_send_native")
					.map_err(|_| RootError::OutOfGas)?;

				T::NativeAsset::transfer(
					&from_account,
					&to_account,
					amount.saturated_into(),
					ExistenceRequirement::KeepAlive,
				)
				.map_err(|_| RootError::InsufficientFunds)?;
			} else {
				ctx.gas_meter()
					.consume_gas(GasInfo::<T>::msg_send_asset(), "msg_send_asset")
					.map_err(|_| RootError::OutOfGas)?;

				// XXX: Need a general way to handle non-unified account
				if frame_system::Account::<T>::get(&to_account).nonce.is_zero() {
					return Err(RootError::UnknownAddress.into());
				}
				let asset_id = T::AssetToDenom::try_convert_back(amt.denom.clone())
					.map_err(|_| RootError::InvalidCoins)?;
				T::Assets::transfer(
					asset_id,
					&from_account,
					&to_account,
					amount.saturated_into(),
					Preservation::Preserve,
				)
				.map_err(|_| RootError::InsufficientFunds)?;
			}
		}

		let event = CosmosEvent {
			r#type: EVENT_TYPE_TRANSFER.into(),
			attributes: vec![
				EventAttribute { key: ATTRIBUTE_KEY_SENDER.into(), value: from_address.into() },
				EventAttribute { key: ATTRIBUTE_KEY_RECIPIENT.into(), value: to_address.into() },
				EventAttribute {
					key: ATTRIBUTE_KEY_AMOUNT.into(),
					value: amount.to_string().into(),
				},
			],
		};
		ctx.event_manager().emit_event(event);

		Ok(())
	}
}
