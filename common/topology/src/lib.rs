// Copyright 2020 Nym Technologies SA
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::filter::VersionFilterable;
use itertools::Itertools;
use nymsphinx_types::{Node as SphinxNode, NodeAddressBytes};
use rand::seq::IteratorRandom;
use std::cmp::max;
use std::collections::HashMap;

pub mod coco;
mod filter;
pub mod gateway;
pub mod mix;
pub mod provider;

// TODO: Figure out why 'Clone' was required to have 'TopologyAccessor<T>' working
// even though it only contains an Arc
pub trait NymTopology: Sized + std::fmt::Debug + Send + Sync + Clone {
    fn new_from_nodes(
        mix_nodes: Vec<mix::Node>,
        mix_provider_nodes: Vec<provider::Node>,
        coco_nodes: Vec<coco::Node>,
        gateway_nodes: Vec<gateway::Node>,
    ) -> Self;
    fn mix_nodes(&self) -> Vec<mix::Node>;
    fn providers(&self) -> Vec<provider::Node>;
    fn gateways(&self) -> Vec<gateway::Node>;
    fn coco_nodes(&self) -> Vec<coco::Node>;
    fn make_layered_topology(&self) -> Result<HashMap<u64, Vec<mix::Node>>, NymTopologyError> {
        let mut layered_topology: HashMap<u64, Vec<mix::Node>> = HashMap::new();
        let mut highest_layer = 0;
        for mix in self.mix_nodes() {
            // we need to have extra space for provider
            if mix.layer > nymsphinx_types::MAX_PATH_LENGTH as u64 {
                return Err(NymTopologyError::InvalidMixLayerError);
            }
            highest_layer = max(highest_layer, mix.layer);

            let layer_nodes = layered_topology.entry(mix.layer).or_insert_with(Vec::new);
            layer_nodes.push(mix);
        }

        // verify the topology - make sure there are no gaps and there is at least one node per layer
        let mut missing_layers = Vec::new();
        for layer in 1..=highest_layer {
            if !layered_topology.contains_key(&layer) {
                missing_layers.push(layer);
            }
            if layered_topology[&layer].is_empty() {
                missing_layers.push(layer);
            }
        }

        if !missing_layers.is_empty() {
            return Err(NymTopologyError::MissingLayerError(missing_layers));
        }

        Ok(layered_topology)
    }

    // Tries to get a route through the mix network
    fn random_mix_route(&self) -> Result<Vec<SphinxNode>, NymTopologyError> {
        let mut layered_topology = self.make_layered_topology()?;
        let num_layers = layered_topology.len();
        let route = (1..=num_layers as u64)
            // unwrap is safe for 'remove' as it it failed, it implied the entry never existed
            // in the map in the first place which would contradict what we've just done
            .map(|layer| layered_topology.remove(&layer).unwrap()) // for each layer
            .map(|nodes| nodes.into_iter().choose(&mut rand::thread_rng()).unwrap()) // choose random node
            .map(|random_node| random_node.into()) // and convert it into sphinx specific node format
            .collect();

        Ok(route)
    }

    fn gateway_exists(&self, gateway_address: &NodeAddressBytes) -> bool {
        let b58_address = gateway_address.to_base58_string();
        self.gateways()
            .iter()
            .find(|&gateway| gateway.identity_key == b58_address)
            .is_some()
    }

    fn random_route_to_gateway(
        &self,
        gateway_address: &NodeAddressBytes,
    ) -> Result<Vec<SphinxNode>, NymTopologyError> {
        let b58_address = gateway_address.to_base58_string();

        let gateway = self
            .gateways()
            .iter()
            .find(|&gateway| gateway.identity_key == b58_address)
            .ok_or_else(|| NymTopologyError::NonExistentGatewayError)?
            .clone();

        Ok(self
            .random_mix_route()?
            .into_iter()
            .chain(std::iter::once(gateway.into()))
            .collect())
    }

    fn all_paths(&self) -> Result<Vec<Vec<SphinxNode>>, NymTopologyError> {
        let mut layered_topology = self.make_layered_topology()?;
        let gateways = self.gateways();

        let sorted_layers: Vec<Vec<SphinxNode>> = (1..=layered_topology.len() as u64)
            .map(|layer| layered_topology.remove(&layer).unwrap()) // get all nodes per layer
            .map(|layer_nodes| layer_nodes.into_iter().map(|node| node.into()).collect()) // convert them into 'proper' sphinx nodes
            .chain(std::iter::once(
                gateways.into_iter().map(|node| node.into()).collect(),
            )) // append all gateways to the end
            .collect();

        let all_paths = sorted_layers
            .into_iter()
            .multi_cartesian_product() // create all possible paths through that
            .collect();

        Ok(all_paths)
    }

    fn filter_system_version(&self, expected_version: &str) -> Self {
        self.filter_node_versions(
            expected_version,
            expected_version,
            expected_version,
            expected_version,
        )
    }

    fn filter_node_versions(
        &self,
        expected_mix_version: &str,
        expected_provider_version: &str,
        expected_gateway_version: &str,
        expected_coco_version: &str,
    ) -> Self {
        let mixes = self.mix_nodes().filter_by_version(expected_mix_version);
        let providers = self
            .providers()
            .filter_by_version(expected_provider_version);
        let gateways = self.gateways().filter_by_version(expected_gateway_version);
        let cocos = self.coco_nodes().filter_by_version(expected_coco_version);

        Self::new_from_nodes(mixes, providers, cocos, gateways)
    }

    fn can_construct_path_through(&self) -> bool {
        !self.mix_nodes().is_empty()
            && !self.gateways().is_empty()
            && self.make_layered_topology().is_ok()
    }
}

#[derive(Debug)]
pub enum NymTopologyError {
    InvalidMixLayerError,
    MissingLayerError(Vec<u64>),
    NonExistentGatewayError,
}
