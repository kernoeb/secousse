import ReactDOM from "react-dom/client";
import App from "./App";
import PopoutApp from "./PopoutApp";
import { getPopoutChannel } from "./lib/utils";
import "./styles.css";

const popoutChannel = getPopoutChannel();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  popoutChannel ? <PopoutApp channel={popoutChannel} /> : <App />
);
