import React from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter, Route, Routes, useNavigate } from "react-router";
import Home from "./Home";
import StructureView from "./StructureView";
import { Box, Button, ButtonGroup, Container } from "@mui/material";
import ComponentView from "./ComponentView";
import "./main.css"

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>

    <BrowserRouter>
      <GobackButton></GobackButton>
      <Container sx={{paddingY: 2}}>
        <Routes>
          <Route index element={<Home />}></Route>
          <Route path="/structure" element={<StructureView />}></Route>
          <Route path="/component" element={<ComponentView />}></Route>
        </Routes>
      </Container>
    </BrowserRouter>
  </React.StrictMode>,
);

export function GobackButton() {
  const navigate = useNavigate();
  return <Box sx={{ position: "sticky", top: 0, zIndex: 100, background: "white", padding: 1, boxShadow: 1 }}>
    <ButtonGroup variant="contained">
      <Button onClick={() => navigate(-1)}>←</Button>
      <Button onClick={() => navigate("/")}>🏠</Button>
      <Button onClick={() => navigate(1)}>→</Button>
    </ButtonGroup></Box>
}
