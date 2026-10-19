<script>
import { onMount } from "svelte";

let count = 0;
let message = "Hello, FlowLens!";

onMount(() => {
  console.log("FlowLens app mounted");
});
</script>

<div class="app-container">
  <h1>{message}</h1>
  <p>Count: {count}</p>
  <button on:click={() => count++}>Increment</button>
</div>

<style>
.app-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100vh;
  font-family: Arial, sans-serif;
  background-color: #f5f5f5;
}

h1 {
  color: #333;
  font-size: 2.5rem;
}

p {
  font-size: 1.5rem;
  margin: 20px 0;
}

button {
  padding: 10px 20px;
  font-size: 1rem;
  background-color: #007bff;
  color: white;
  border: none;
  border-radius: 5px;
  cursor: pointer;
}

button:hover {
  background-color: #0056b3;
}
</style>