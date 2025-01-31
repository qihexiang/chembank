import { Box, Button, ButtonGroup, Checkbox, FormControlLabel, Grid2, Slider, TextField, Typography } from "@mui/material";
import { useState } from "react";
import { useNavigate, useSearchParams } from "react-router";
import { createStructure, searchStructure, setComponent, structureCount } from "./bindings";
import useFetch from "./useFetch";



export default function ComponentView() {
    const navigate = useNavigate();
    const [searchParams] = useSearchParams();
    const page = Number(searchParams.get("page") ?? "0");
    const componentOf = Number(searchParams.get("component_of")!);
    const [componentCount, setComponentCount] = useState(1);
    const [keyword, setKeyword] = useState<string | null>(null)
    const [[minCharge, maxCharge], setChargeRange] = useState<[number, number]>([-10, 10])
    const [structures] = useFetch(() => searchStructure(100, page, keyword, maxCharge, minCharge), [], [page, keyword, minCharge, maxCharge]);
    const [count] = useFetch(() => structureCount(), 0, [structures])
    const [selected, setSelected] = useState<number | null>(null);
    return <Grid2 display={"flex"} flexDirection={"column"} gap={1}>
        <Grid2 container spacing={2}>
            <Grid2 spacing={1} container alignItems={"center"} justifyContent={"start"} size={12}>
                <Typography variant="h4">添加子结构</Typography>
                <TextField type="number" value={componentCount} onChange={(e) => setComponentCount(Number(e.target.value))} label="子结构数量"></TextField>
                <ButtonGroup variant="contained">
                    <Button color="primary" disabled={selected === null} onClick={async () => {
                        await setComponent(componentOf, selected!, componentCount)
                        navigate(`/structure?id=${componentOf}`)
                    }}>添加子结构并返回</Button>
                    <Button color="success" onClick={async () => {
                        const id = await createStructure(null, "", null, 0);
                        navigate(`/structure?id=${id}&component_of=${componentOf}`)
                    }}>新建结构作为子结构</Button>
                    <Button color="error" onClick={() => navigate(`/structure?id=${componentOf}`)}>取消并返回</Button>
                </ButtonGroup>
            </Grid2>
            <Grid2 container flexDirection={"row"} size={12} spacing={2}>
                <TextField label="关键词" value={keyword ?? ""} onChange={(e) => { if (e.target.value === "") { setKeyword(null) } else { setKeyword(e.target.value) } }}></TextField>
                <Box sx={{ width: 192 }}>
                    <Typography id="charge_range_label">电荷范围</Typography>
                    <Slider aria-labelledby="charge_range_label" min={-10} max={10} valueLabelDisplay="auto" value={[minCharge, maxCharge]} onChange={(_, v) => setChargeRange(v as [number, number])}></Slider>
                </Box>
            </Grid2>
        </Grid2>
        <Grid2>
            <Grid2 spacing={1} container justifyContent={"center"}>
                <Grid2 size={1}>
                    <Typography variant="h6">序号</Typography>
                </Grid2>
                <Grid2 size={2}>
                    <Typography variant="h6">名称</Typography>
                </Grid2>
                <Grid2 size={3}>
                    <Typography variant="h6">分子式</Typography>
                </Grid2>
                <Grid2 size={3}>
                    <Typography variant="h6">SMILES</Typography>
                </Grid2>
                <Grid2 size={1}>
                    <Typography variant="h6">电荷数</Typography>
                </Grid2>
                <Grid2 size={2}>
                    <Typography variant="h6">操作</Typography>
                </Grid2>
            </Grid2>
        </Grid2>
        <Grid2 display={"flex"} gap={1} flexDirection={"column"} justifyContent={"space-around"}>
            {
                structures.map((structure, idx) => <Grid2 container justifyContent={"center"} key={idx}>
                    <Grid2 size={1}>{structure.id}</Grid2>
                    <Grid2 size={2}>{structure.name}</Grid2>
                    <Grid2 size={3}>{structure.formula}</Grid2>
                    <Grid2 size={3}>{structure.smiles}</Grid2>
                    <Grid2 size={1}>{structure.charge}</Grid2>
                    <Grid2 size={2}>
                        <FormControlLabel control={<Checkbox checked={selected === structure.id} onChange={(e) => {
                            if (e.target.checked) {
                                setSelected(structure.id)
                            } else {
                                setSelected(null)
                            }
                        }} />} label="选中" />
                    </Grid2>
                </Grid2>)
            }
        </Grid2>
        <Grid2>
            <ButtonGroup variant="contained">
                {
                    new Array(Math.floor(count / 100) + 1).fill(0).map((_, k) => k).map(key => <Button color={key === page ? "info" : "primary"} key={key} onClick={() => {
                        navigate(`/?page=${key}&component_of=${componentOf}`)
                    }}>
                        {key + 1}
                    </Button>)
                }
            </ButtonGroup>
        </Grid2>
    </Grid2>
}