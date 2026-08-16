import { definePlugin } from "@decky/api";
import { BsCpuFill } from "react-icons/bs";
import App from "./App";

export default definePlugin(() => {
  return {
    name: "AM02 Decky",
    content: <App />,
    icon: <BsCpuFill />,
    onDismount: () => {},
  };
});
