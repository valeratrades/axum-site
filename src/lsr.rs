use v_exchanges::Binance;

#[tokio::main]
async fn main() {
	let bn = Binance::default();
	let lsr = bn.global_lsr_account(("BTC", "USDT").into(), "5m".into(), 24 * 12 + 1, "Global".into()).await.unwrap();
	dbg!(&lsr[..5]);
}
