import React from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter, Route, Routes } from "react-router";
import Home from "./Home";
import StructureView from "./StructureView";
import { Container } from "@mui/material";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Container>
      <BrowserRouter>
        <Routes>
          <Route index element={<Home />}></Route>
          <Route path="/structure" element={<StructureView />}></Route>
        </Routes>
      </BrowserRouter>
    </Container>
  </React.StrictMode>,
);
