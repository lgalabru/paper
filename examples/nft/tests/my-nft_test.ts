import {
  Clarinet,
  Tx,
  Chain,
  Account,
  types,
} from "https://deno.land/x/clarinet@v0.12.0/index.ts";
import { assertEquals } from "https://deno.land/std@0.90.0/testing/asserts.ts";

Clarinet.test({
  name: "Ensure that NFT token URL and ID is as expected",
  async fn(chain: Chain, accounts: Map<string, Account>) {
    let wallet_1 = accounts.get("wallet_1")!;
    let block = chain.mineBlock([
      Tx.contractCall("my-nft", "get-last-token-id", [], wallet_1.address),
      Tx.contractCall(
        "my-nft",
        "get-token-uri",
        [types.uint(1)],
        wallet_1.address
      ),
    ]);
    assertEquals(block.receipts.length, 2);
    assertEquals(block.height, 2);
    block.receipts[0].result.expectOk().expectUint(0);
    block.receipts[1].result
      .expectOk()
      .expectSome()
      .expectAscii("https://docs.stacks.co");
  },
});
