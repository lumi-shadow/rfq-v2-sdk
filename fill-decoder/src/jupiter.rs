use anchor_lang::{declare_program, Discriminator};

declare_program!(jupiter);

pub use jupiter::client::args::*;
pub use jupiter::types::*;
pub use jupiter::ID as JUPITER_PROGRAM_ID;

pub fn route_discriminator() -> &'static [u8] {
    <Route as Discriminator>::DISCRIMINATOR
}

pub fn route_v2_discriminator() -> &'static [u8] {
    <RouteV2 as Discriminator>::DISCRIMINATOR
}

pub fn shared_accounts_route_discriminator() -> &'static [u8] {
    <SharedAccountsRoute as Discriminator>::DISCRIMINATOR
}

pub fn shared_accounts_route_v2_discriminator() -> &'static [u8] {
    <SharedAccountsRouteV2 as Discriminator>::DISCRIMINATOR
}

pub fn is_jupiter_route(data: &[u8]) -> bool {
    data.starts_with(route_discriminator())
        || data.starts_with(route_v2_discriminator())
        || data.starts_with(shared_accounts_route_discriminator())
        || data.starts_with(shared_accounts_route_v2_discriminator())
}

#[derive(Debug, Clone)]
pub enum DecodedJupiterRoute {
    Route {
        route_plan: Vec<RoutePlanStep>,
        in_amount: u64,
        quoted_out_amount: u64,
        slippage_bps: u16,
        platform_fee_bps: u8,
    },
    RouteV2 {
        in_amount: u64,
        quoted_out_amount: u64,
        slippage_bps: u16,
        platform_fee_bps: u16,
        positive_slippage_bps: u16,
        route_plan: Vec<RoutePlanStepV2>,
    },
    SharedAccountsRoute {
        id: u8,
        route_plan: Vec<RoutePlanStep>,
        in_amount: u64,
        quoted_out_amount: u64,
        slippage_bps: u16,
        platform_fee_bps: u8,
    },
    SharedAccountsRouteV2 {
        id: u8,
        in_amount: u64,
        quoted_out_amount: u64,
        slippage_bps: u16,
        platform_fee_bps: u16,
        positive_slippage_bps: u16,
        route_plan: Vec<RoutePlanStepV2>,
    },
}

impl DecodedJupiterRoute {
    pub fn decode(data: &[u8]) -> Option<Self> {
        use anchor_lang::AnchorDeserialize;

        if data.starts_with(route_discriminator()) {
            let mut payload = &data[route_discriminator().len()..];
            let args = Route::deserialize(&mut payload).ok()?;
            return Some(DecodedJupiterRoute::Route {
                route_plan: args.route_plan,
                in_amount: args.in_amount,
                quoted_out_amount: args.quoted_out_amount,
                slippage_bps: args.slippage_bps,
                platform_fee_bps: args.platform_fee_bps,
            });
        }
        if data.starts_with(route_v2_discriminator()) {
            let mut payload = &data[route_v2_discriminator().len()..];
            let args = RouteV2::deserialize(&mut payload).ok()?;
            return Some(DecodedJupiterRoute::RouteV2 {
                in_amount: args.in_amount,
                quoted_out_amount: args.quoted_out_amount,
                slippage_bps: args.slippage_bps,
                platform_fee_bps: args.platform_fee_bps,
                positive_slippage_bps: args.positive_slippage_bps,
                route_plan: args.route_plan,
            });
        }
        if data.starts_with(shared_accounts_route_discriminator()) {
            let mut payload = &data[shared_accounts_route_discriminator().len()..];
            let args = SharedAccountsRoute::deserialize(&mut payload).ok()?;
            return Some(DecodedJupiterRoute::SharedAccountsRoute {
                id: args.id,
                route_plan: args.route_plan,
                in_amount: args.in_amount,
                quoted_out_amount: args.quoted_out_amount,
                slippage_bps: args.slippage_bps,
                platform_fee_bps: args.platform_fee_bps,
            });
        }
        if data.starts_with(shared_accounts_route_v2_discriminator()) {
            let mut payload = &data[shared_accounts_route_v2_discriminator().len()..];
            let args = SharedAccountsRouteV2::deserialize(&mut payload).ok()?;
            return Some(DecodedJupiterRoute::SharedAccountsRouteV2 {
                id: args.id,
                in_amount: args.in_amount,
                quoted_out_amount: args.quoted_out_amount,
                slippage_bps: args.slippage_bps,
                platform_fee_bps: args.platform_fee_bps,
                positive_slippage_bps: args.positive_slippage_bps,
                route_plan: args.route_plan,
            });
        }
        None
    }

    pub fn in_amount(&self) -> u64 {
        match self {
            DecodedJupiterRoute::Route { in_amount, .. } => *in_amount,
            DecodedJupiterRoute::RouteV2 { in_amount, .. } => *in_amount,
            DecodedJupiterRoute::SharedAccountsRoute { in_amount, .. } => *in_amount,
            DecodedJupiterRoute::SharedAccountsRouteV2 { in_amount, .. } => *in_amount,
        }
    }

    pub fn kind_name(&self) -> &'static str {
        match self {
            DecodedJupiterRoute::Route { .. } => "route",
            DecodedJupiterRoute::RouteV2 { .. } => "route_v2",
            DecodedJupiterRoute::SharedAccountsRoute { .. } => "shared_accounts_route",
            DecodedJupiterRoute::SharedAccountsRouteV2 { .. } => "shared_accounts_route_v2",
        }
    }

    pub fn quoted_out_amount(&self) -> u64 {
        match self {
            DecodedJupiterRoute::Route {
                quoted_out_amount, ..
            } => *quoted_out_amount,
            DecodedJupiterRoute::RouteV2 {
                quoted_out_amount, ..
            } => *quoted_out_amount,
            DecodedJupiterRoute::SharedAccountsRoute {
                quoted_out_amount, ..
            } => *quoted_out_amount,
            DecodedJupiterRoute::SharedAccountsRouteV2 {
                quoted_out_amount, ..
            } => *quoted_out_amount,
        }
    }

    pub fn slippage_bps(&self) -> u16 {
        match self {
            DecodedJupiterRoute::Route { slippage_bps, .. } => *slippage_bps,
            DecodedJupiterRoute::RouteV2 { slippage_bps, .. } => *slippage_bps,
            DecodedJupiterRoute::SharedAccountsRoute { slippage_bps, .. } => *slippage_bps,
            DecodedJupiterRoute::SharedAccountsRouteV2 { slippage_bps, .. } => *slippage_bps,
        }
    }

    pub fn platform_fee_bps(&self) -> u16 {
        match self {
            DecodedJupiterRoute::Route {
                platform_fee_bps, ..
            } => *platform_fee_bps as u16,
            DecodedJupiterRoute::RouteV2 {
                platform_fee_bps, ..
            } => *platform_fee_bps,
            DecodedJupiterRoute::SharedAccountsRoute {
                platform_fee_bps, ..
            } => *platform_fee_bps as u16,
            DecodedJupiterRoute::SharedAccountsRouteV2 {
                platform_fee_bps, ..
            } => *platform_fee_bps,
        }
    }

    pub fn positive_slippage_bps(&self) -> Option<u16> {
        match self {
            DecodedJupiterRoute::Route { .. } => None,
            DecodedJupiterRoute::RouteV2 {
                positive_slippage_bps,
                ..
            } => Some(*positive_slippage_bps),
            DecodedJupiterRoute::SharedAccountsRoute { .. } => None,
            DecodedJupiterRoute::SharedAccountsRouteV2 {
                positive_slippage_bps,
                ..
            } => Some(*positive_slippage_bps),
        }
    }

    pub fn extract_rfq_v2_fill_data(&self) -> Option<(Side, &[u8])> {
        match self {
            DecodedJupiterRoute::Route { route_plan, .. } => {
                for step in route_plan {
                    if let Swap::JupiterRfqV2 { side, fill_data } = &step.swap {
                        return Some((*side, fill_data));
                    }
                }
                None
            }
            DecodedJupiterRoute::RouteV2 { route_plan, .. } => {
                for step in route_plan {
                    if let Swap::JupiterRfqV2 { side, fill_data } = &step.swap {
                        return Some((*side, fill_data));
                    }
                }
                None
            }
            DecodedJupiterRoute::SharedAccountsRoute { route_plan, .. } => {
                for step in route_plan {
                    if let Swap::JupiterRfqV2 { side, fill_data } = &step.swap {
                        return Some((*side, fill_data));
                    }
                }
                None
            }
            DecodedJupiterRoute::SharedAccountsRouteV2 { route_plan, .. } => {
                for step in route_plan {
                    if let Swap::JupiterRfqV2 { side, fill_data } = &step.swap {
                        return Some((*side, fill_data));
                    }
                }
                None
            }
        }
    }

    pub fn route_plan_v1(&self) -> Option<&[RoutePlanStep]> {
        match self {
            DecodedJupiterRoute::Route { route_plan, .. } => Some(route_plan),
            DecodedJupiterRoute::SharedAccountsRoute { route_plan, .. } => Some(route_plan),
            _ => None,
        }
    }

    pub fn route_plan_v2(&self) -> Option<&[RoutePlanStepV2]> {
        match self {
            DecodedJupiterRoute::RouteV2 { route_plan, .. } => Some(route_plan),
            DecodedJupiterRoute::SharedAccountsRouteV2 { route_plan, .. } => Some(route_plan),
            _ => None,
        }
    }
}

pub fn swap_kind_name(swap: &Swap) -> String {
    let dbg = format!("{swap:?}");
    let end = dbg.find([' ', '{', '(']).unwrap_or(dbg.len());
    dbg[..end].to_string()
}
