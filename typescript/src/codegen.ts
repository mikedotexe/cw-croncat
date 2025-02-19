import codegen from '@cosmwasm/ts-codegen';
import path from 'path';
import fs from "fs";

enum OutputType {
    contracts = "contracts",
    agents = "croncat-agents",
    factory = "croncat-factory",
    manager = "croncat-manager",
    tasks = "croncat-tasks",
    modulebalances = "mod-balances",
    moduledao = "mod-dao",
    modulegeneric = "mod-generic",
    modulenft = "mod-nft",
    packages = "packages",
}

export type CompilationSpec = {
    contractName: string;
    schemaDir: string;
    outputPath: string;
    outputType: OutputType;
};

const CONTRACTS_OUTPUT_DIR = ".";
const DEFAULT_CONFIG = {
    schemaRoots: [
        {
            name: OutputType.contracts,
            paths: [`../${OutputType.contracts}`],
            outputName: OutputType.contracts,
            outputDir: CONTRACTS_OUTPUT_DIR,
        },
        {
            name: OutputType.contracts,
            paths: [`../contracts/${OutputType.agents}`],
            outputName: OutputType.agents,
            outputDir: CONTRACTS_OUTPUT_DIR,
        },
        {
            name: OutputType.contracts,
            paths: [`../contracts/${OutputType.factory}`],
            outputName: OutputType.factory,
            outputDir: CONTRACTS_OUTPUT_DIR,
        },
        {
            name: OutputType.contracts,
            paths: [`../contracts/${OutputType.manager}`],
            outputName: OutputType.manager,
            outputDir: CONTRACTS_OUTPUT_DIR,
        },
        {
            name: OutputType.contracts,
            paths: [`../contracts/${OutputType.tasks}`],
            outputName: OutputType.tasks,
            outputDir: CONTRACTS_OUTPUT_DIR,
        },
        {
            name: OutputType.contracts,
            paths: [`../contracts/${OutputType.modulebalances}`],
            outputName: OutputType.modulebalances,
            outputDir: CONTRACTS_OUTPUT_DIR,
        },
        {
            name: OutputType.contracts,
            paths: [`../contracts/${OutputType.modulegeneric}`],
            outputName: OutputType.modulegeneric,
            outputDir: CONTRACTS_OUTPUT_DIR,
        },
        {
            name: OutputType.contracts,
            paths: [`../contracts/${OutputType.moduledao}`],
            outputName: OutputType.moduledao,
            outputDir: CONTRACTS_OUTPUT_DIR,
        },
        {
            name: OutputType.contracts,
            paths: [`../contracts/${OutputType.modulenft}`],
            outputName: OutputType.modulenft,
            outputDir: CONTRACTS_OUTPUT_DIR,
        },
        {
            name: OutputType.packages,
            paths: [`../${OutputType.packages}`],
            outputName: OutputType.packages,
            outputDir: CONTRACTS_OUTPUT_DIR,
        },
    ],
};


async function generateTs(spec: CompilationSpec): Promise<void> {
    const out = `${spec.outputPath}/${spec.outputType}/${spec.contractName}`;
    const name = spec.contractName;
    console.log(spec.schemaDir);
    return await codegen({
        contracts: [
            {
                name: `${name}`,
                dir: `${spec.schemaDir}`,
            },
        ],
        outPath: `./${OutputType.contracts}/${name}`,
    }).then(() => {
        console.log(`${name} done!`);
    });
}

function getSchemaDirectories(
    rootDir: string,
): Promise<string[][]> {
    return new Promise((resolve, _reject) => {
        const directories: string[][] = [];
        // get all the schema directories in all the root dir
        fs.readdir(rootDir, (err: any, dirEntries: any[]) => {
            if (err) {
                console.error(err);
                return;
            }
            if (!dirEntries) {
                console.warn(`no entries found in ${rootDir}`);
                resolve([]);
                return;
            }
            dirEntries.forEach((entry) => {
                try {
                    const schemaDir = path.resolve(rootDir, entry, "schema");
                    if (
                        fs.existsSync(schemaDir) &&
                        fs.lstatSync(schemaDir).isDirectory()
                    ) {
                        directories.push([schemaDir.replaceAll("\\", "/"), entry]);
                    }
                } catch (e) {
                    console.warn(e);
                }
            });
            resolve(directories);
        });
    });
}

async function main() {
    let config = {
        ...DEFAULT_CONFIG,
    };

    const compilationSpecs: CompilationSpec[] = [];
    console.log("Calculating generation specs...");
    for (const root of config.schemaRoots) {
        const { name, paths, outputName, outputDir } = root;
        for (const path of paths) {
            const schemaDirectories = await getSchemaDirectories(path);
            console.log(schemaDirectories)
            for (const [directory, contractName] of schemaDirectories) {
                compilationSpecs.push({
                    contractName: contractName,
                    schemaDir: directory,
                    outputPath: outputDir,
                    outputType: outputName,
                });
            }
        }
    }
    console.log(`code generating for ${compilationSpecs?.length ?? 0} specs...`);

    const codegenResponses: Promise<void>[] = [];
    for (const spec of compilationSpecs) {
        codegenResponses.push(generateTs(spec));
    }
    await Promise.all(codegenResponses);

    console.log(`code generation complete`);
}

main();