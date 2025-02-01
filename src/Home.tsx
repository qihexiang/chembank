import { Box, Button, ButtonGroup, Grid2, Slider, TextField, Typography } from "@mui/material";
import { useState } from "react";
import { useNavigate, useSearchParams } from "react-router";
import { createStructure, importFromFolder, removeStructure, resetDatabase, searchStructure, structureCount } from "./bindings";
import useFetch from "./useFetch";
import { confirm, message, open, save } from "@tauri-apps/api/dialog";



export default function Home() {
    const navigate = useNavigate();
    const [searchParams] = useSearchParams();
    const page = Number(searchParams.get("page") ?? "0");
    const [keyword, setKeyword] = useState<string | null>(null)
    const [[minCharge, maxCharge], setChargeRange] = useState<[number, number]>([-10, 10])
    const [structures, refreshList] = useFetch(() => searchStructure(100, page, keyword, maxCharge, minCharge), [], [page, keyword, minCharge, maxCharge]);
    const [count] = useFetch(() => structureCount(), 0, [structures])
    return <Grid2 display={"flex"} flexDirection={"column"} gap={1}>
        <Grid2 container spacing={2}>
            <Grid2 spacing={1} container alignItems={"center"} justifyContent={"start"} size={12}>
                <Typography variant="h4">ChemBank</Typography>
                <Button variant="contained" color="success" onClick={() => createStructure(null, "", null, 0).then(id => navigate(`/structure?id=${id}`))}>新建结构</Button>
                <Button variant="contained" color="primary" onClick={async () => {
                    const folder = await open({
                        directory: true,
                        filters: [
                            { name: "文件夹", extensions: [] }
                        ]
                    })
                    if (folder !== null) {
                        const resetDB = await confirm("导入数据前，是否需要清空数据库？点击确定清空数据库，点击取消在保留现有数据的情况下导入。")
                        if (resetDB) {
                            await resetDatabase()
                        }
                        importFromFolder(folder as string)
                            .then(() => message("导入成功"))
                            .then(refreshList)
                            .catch((e) => message(`导入失败，原因为：${e}`))
                    }
                }}>导入数据</Button>
                <Button variant="contained" color="secondary" onClick={async () => {
                    const folder = await save({
                        filters: [
                            {
                                name: "文件夹",
                                extensions: []
                            }
                        ]
                    });
                    if (folder !== null) {
                        navigate(`/export?folder=${folder}`)
                    }
                }}>导出数据</Button>
                <Button variant="contained" color="error" onClick={async () => {
                    await resetDatabase();
                    refreshList()
                }}>清空数据</Button>
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
                        <ButtonGroup variant="contained">
                            <Button onClick={() => navigate(`/structure?id=${structure.id}`)}>详情</Button>
                            <Button color="error" onClick={() => {
                                removeStructure(structure.id).then(refreshList)
                            }}>删除</Button>
                        </ButtonGroup>
                    </Grid2>
                </Grid2>)
            }
        </Grid2>
        <Grid2>
            <ButtonGroup variant="contained">
                {
                    new Array(Math.floor(count / 100) + 1).fill(0).map((_, k) => k).map(key => <Button color={key === page ? "info" : "primary"} key={key} onClick={() => {
                        navigate(`/?page=${key}`)
                    }}>
                        {key + 1}
                    </Button>)
                }
            </ButtonGroup>
        </Grid2>
    </Grid2 >
}