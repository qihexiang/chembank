import { Box, Button, Typography } from "@mui/material";
import { useEffect, useState } from "react";
import { useNavigate, useSearchParams } from "react-router";
import { exportToFolder } from "./bindings";

export default function ExportView() {
    const [finished, setFinished] = useState<number | string>(0);
    const [searchParams] = useSearchParams();
    const folder = searchParams.get("folder")!;
    const navigate = useNavigate();

    useEffect(() => {
        exportToFolder(folder).then(() => setFinished(1)).catch((e) => setFinished(String(e)))
    }, [])

    return <Box>
        {
            finished === 0 ? <Typography>正在导出数据到{folder}</Typography> :
                finished === 1 ? <>
                    <Typography>数据导出完成</Typography>
                    <Button variant="contained" color="success" onClick={() => navigate("/")}>返回首页</Button>
                </> :
                    <>
                        <Typography>数据导出出错。</Typography>
                        <Typography>{finished}</Typography>
                        <Button variant="contained" color="info" onClick={() => navigate("/")}>返回首页</Button>
                    </>
        }
    </Box>
}