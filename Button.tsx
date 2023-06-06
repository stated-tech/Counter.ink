import React, { useState } from "react";
import { ApiPromise, WsProvider } from "@polkadot/api";
import { ContractPromise } from "@polkadot/api-contract";
import contractAbi from "./Contract.json";

const SwapButton = () => {
  const [error, setError] = useState(null);

  const createOrder = async () => {
    // Connection
    const provider = new WsProvider("ws://127.0.0.1:9944");
    const api = await ApiPromise.create({ provider });

    // Contract
    const contractAddress = ""; // To modify
    const contract = new ContractPromise(api, contractAbi, contractAddress);

    // Account sender
    const sender = ""; // To modify

    // Parameters
    const token_a = ""; // Contract A address"
    const token_b = ""; // Contract B address"
    const amount_a = 100;
    const amount_b = 200;
    const duration = 3600;

    try {
      // Call the `create_order` function
      const { result, output } = await contract.tx
        .createOrder(
          { value: 0, gasLimit: 5000000 },
          token_a,
          token_b,
          amount_a,
          amount_b,
          duration
        )
        .signAndSend(sender);

      if (result.isErr) {
        throw new Error(`${output.toHuman()}`);
      }

      // Clear
      setError(null);
    } catch (err) {
      setError(err.message);
    }
  };

  return (
    <div>
      <button onClick={createOrder}>Create Order</button>
      {error && <p>Error: {error}</p>}
    </div>
  );
};

export default SwapButton;






