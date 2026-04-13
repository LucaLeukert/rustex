import fs from "node:fs";
import path from "node:path";
import { CliConfig, Options } from "@effect/cli";
import { BunContext, BunRuntime } from "@effect/platform-bun";
import { Data, Effect, Layer, Option, pipe } from "effect";
import ts from "typescript";

type Severity = "error" | "warning" | "note";
type Visibility = "public" | "internal";
type FunctionKind = "query" | "mutation" | "action";
type ContractProvenance = "validator" | "generated_ts" | "inferred" | "missing";

type Origin = {
  file: string;
  line: number;
  column: number;
};

type Diagnostic = {
  code: string;
  severity: Severity;
  message: string;
  symbol: string | null;
  provenance: string | null;
  suggestion: string | null;
  primary_span: Origin | null;
  related_spans: Origin[];
};

type Field = {
  name: string;
  required: boolean;
  type: TypeNode;
  doc: string | null;
  source: Origin | null;
};

type TypeNode =
  | { kind: "string" }
  | { kind: "float64" }
  | { kind: "int64" }
  | { kind: "boolean" }
  | { kind: "null" }
  | { kind: "bytes" }
  | { kind: "any" }
  | { kind: "literal_string"; value: string }
  | { kind: "literal_number"; value: number }
  | { kind: "literal_boolean"; value: boolean }
  | { kind: "id"; table: string }
  | { kind: "array"; element: TypeNode }
  | { kind: "record"; value: TypeNode }
  | { kind: "object"; fields: Field[]; open: boolean }
  | { kind: "union"; members: TypeNode[] }
  | { kind: "unknown"; reason: string; confidence: number };

type Table = {
  name: string;
  doc_name: string;
  document_type: TypeNode;
  source: Origin | null;
};

type FunctionEntry = {
  canonical_path: string;
  module_path: string;
  export_name: string;
  visibility: Visibility;
  kind: FunctionKind;
  args_type: TypeNode | null;
  returns_type: TypeNode | null;
  contract_provenance: ContractProvenance;
  source: Origin | null;
};

type IrPackage = {
  project: {
    name: string;
    root: string;
    convex_root: string;
    convex_version: string | null;
    generated_metadata_present: boolean;
  };
  tables: Table[];
  functions: FunctionEntry[];
  diagnostics: Diagnostic[];
  manifest_meta: {
    rustex_version: string;
    manifest_version: number;
    input_hash: string;
  };
};

class AnalyzerError extends Data.TaggedError("AnalyzerError")<{
  message: string;
  cause?: unknown;
}> {}

type AnalyzerContext = {
  readonly projectRoot: string;
  readonly convexRoot: string;
  readonly checker: ts.TypeChecker;
  readonly program: ts.Program;
  readonly diagnostics: Diagnostic[];
};

const FUNCTION_KIND_MAP: Record<
  string,
  { kind: FunctionKind; visibility: Visibility }
> = {
  query: { kind: "query", visibility: "public" },
  mutation: { kind: "mutation", visibility: "public" },
  action: { kind: "action", visibility: "public" },
  internalQuery: { kind: "query", visibility: "internal" },
  internalMutation: { kind: "mutation", visibility: "internal" },
  internalAction: { kind: "action", visibility: "internal" },
};

const main = Effect.gen(function* () {
  const args = yield* parseCliArgs(process.argv.slice(2));
  const packageJsonPath = path.join(args.projectRoot, "package.json");
  const convexPackagePath = path.join(
    args.projectRoot,
    "node_modules",
    "convex",
    "package.json",
  );

  const parsedConfig = yield* parseTsConfig(args.convexRoot);
  const fileNames = Array.from(
    new Set([...parsedConfig.fileNames, ...listTsFiles(args.convexRoot)]),
  );
  const program = ts.createProgram({
    rootNames: fileNames,
    options: parsedConfig.options,
  });

  const context: AnalyzerContext = {
    projectRoot: args.projectRoot,
    convexRoot: args.convexRoot,
    checker: program.getTypeChecker(),
    program,
    diagnostics: [],
  };

  const packageJson = readJsonFile(packageJsonPath).pipe(
    Effect.orElseSucceed(() => ({ name: path.basename(args.projectRoot) })),
  );
  const convexVersion = detectConvexVersion(convexPackagePath, args.convexRoot);
  const schemaSource = pipe(
    program.getSourceFiles().find((sf) =>
      normalize(sf.fileName) === normalize(path.join(args.convexRoot, "schema.ts")),
    ),
    Option.fromNullable,
  );

  const tables = pipe(
    schemaSource,
    Option.match({
      onNone: () => [],
      onSome: (source) => extractSchema(context, source),
    }),
  );

  const pkg = yield* packageJson;

  const ir: IrPackage = {
    project: {
      name:
        typeof pkg === "object" &&
        pkg !== null &&
        "name" in pkg &&
        typeof pkg.name === "string"
          ? pkg.name
          : path.basename(args.projectRoot),
      root: normalize(args.projectRoot),
      convex_root: normalize(args.convexRoot),
      convex_version: yield* convexVersion,
      generated_metadata_present: fs.existsSync(
        path.join(args.convexRoot, "_generated", "api.d.ts"),
      ),
    },
    tables,
    functions: extractFunctions(context),
    diagnostics: context.diagnostics,
    manifest_meta: {
      rustex_version: "0.1.0",
      manifest_version: 1,
      input_hash: "",
    },
  };

  yield* writeStdout(JSON.stringify(ir));
}).pipe(
  Effect.catchTag(
    "AnalyzerError",
    (error) =>
      Effect.try({
        try: () => {
          console.error(error.message);
          if (error.cause) {
            console.error(error.cause);
          }
          process.exitCode = 1;
        },
        catch: (cause) =>
          new AnalyzerError({ message: "failed to report analyzer error", cause }),
      }),
  ),
);

BunRuntime.runMain(
  pipe(main, Effect.provide(Layer.mergeAll(CliConfig.defaultLayer, BunContext.layer))),
);

function parseCliArgs(argv: string[]) {
  const parser = Options.all({
    projectRoot: pipe(
      Options.text("project-root"),
      Options.withAlias("p"),
      Options.withDescription("Path to the analyzed project root"),
    ),
    convexRoot: pipe(
      Options.text("convex-root"),
      Options.withAlias("c"),
      Options.withDescription("Path to the Convex root directory"),
    ),
  });

  return pipe(
    Options.processCommandLine(parser, argv, CliConfig.defaultConfig),
    Effect.map(([validationError, leftover, parsed]) => {
      if (Option.isSome(validationError)) {
        throw new AnalyzerError({
          message: "failed to parse analyzer CLI arguments",
          cause: validationError.value,
        });
      }
      if (leftover.length > 0) {
        throw new AnalyzerError({
          message: `unexpected positional arguments: ${leftover.join(" ")}`,
        });
      }
      return {
        projectRoot: path.resolve(parsed.projectRoot),
        convexRoot: path.resolve(parsed.convexRoot),
      };
    }),
    Effect.mapError((cause) =>
      cause instanceof AnalyzerError
        ? cause
        : new AnalyzerError({ message: "failed to parse CLI arguments", cause }),
    ),
  );
}

function parseTsConfig(convexRoot: string) {
  return Effect.try({
    try: () => {
      const tsConfigPath = ts.findConfigFile(
        convexRoot,
        ts.sys.fileExists,
        "tsconfig.json",
      );
      if (!tsConfigPath) {
        return {
          fileNames: listTsFiles(convexRoot),
          options: {
            target: ts.ScriptTarget.ES2022,
            module: ts.ModuleKind.NodeNext,
          },
        };
      }
      const loaded = ts.readConfigFile(tsConfigPath, ts.sys.readFile);
      return ts.parseJsonConfigFileContent(
        loaded.config,
        ts.sys,
        path.dirname(tsConfigPath),
      );
    },
    catch: (cause) =>
      new AnalyzerError({ message: "failed to parse TypeScript config", cause }),
  });
}

function detectConvexVersion(convexPackagePath: string, convexRoot: string) {
  return pipe(
    readJsonFile(convexPackagePath),
    Effect.map((json) => {
      if (
        typeof json === "object" &&
        json !== null &&
        "version" in json &&
        typeof json.version === "string"
      ) {
        return json.version;
      }
      return null;
    }),
    Effect.orElse(() =>
      Effect.try({
        try: () => {
          const generatedPath = path.join(convexRoot, "_generated", "api.d.ts");
          if (!fs.existsSync(generatedPath)) {
            return null;
          }
          const match = fs
            .readFileSync(generatedPath, "utf8")
            .match(/Generated by convex@([0-9A-Za-z.+-]+)/);
          return match?.[1] ?? null;
        },
        catch: (cause) =>
          new AnalyzerError({ message: "failed to detect Convex version", cause }),
      }),
    ),
  );
}

function readJsonFile(filePath: string) {
  return Effect.try({
    try: () => JSON.parse(fs.readFileSync(filePath, "utf8")) as unknown,
    catch: (cause) =>
      new AnalyzerError({
        message: `failed to read JSON file ${filePath}`,
        cause,
      }),
  });
}

function writeStdout(contents: string) {
  return Effect.try({
    try: () => process.stdout.write(contents),
    catch: (cause) =>
      new AnalyzerError({ message: "failed to write analyzer output", cause }),
  });
}

function normalize(value: string): string {
  return value.split(path.sep).join("/");
}

function listTsFiles(root: string): string[] {
  const found: string[] = [];
  walk(root, (file) => {
    if (file.endsWith(".ts") || file.endsWith(".tsx")) {
      found.push(file);
    }
  });
  return found;
}

function walk(dir: string, visit: (file: string) => void): void {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      walk(full, visit);
    } else {
      visit(full);
    }
  }
}

function extractSchema(context: AnalyzerContext, sourceFile: ts.SourceFile): Table[] {
  const tables: Table[] = [];
  sourceFile.forEachChild((node) => {
    if (
      ts.isExportAssignment(node) &&
      ts.isCallExpression(node.expression) &&
      expressionName(node.expression.expression) === "defineSchema"
    ) {
      const schemaArg = deref(context, node.expression.arguments[0]);
      if (schemaArg && ts.isObjectLiteralExpression(schemaArg)) {
        for (const prop of schemaArg.properties) {
          if (!ts.isPropertyAssignment(prop)) continue;
          const tableName = propertyName(prop.name);
          const init = deref(context, prop.initializer);
          if (
            !init ||
            !ts.isCallExpression(init) ||
            expressionName(init.expression) !== "defineTable"
          ) {
            continue;
          }
          const validatorExpr = deref(context, init.arguments[0]);
          tables.push({
            name: tableName,
            doc_name: `${pascalCase(tableName)}Doc`,
            document_type: parseValidator(context, validatorExpr, sourceFile),
            source: origin(sourceFile, prop),
          });
        }
      }
    }
  });
  return tables;
}

function extractFunctions(context: AnalyzerContext): FunctionEntry[] {
  const files = context.program
    .getSourceFiles()
    .filter((sf) =>
      normalize(sf.fileName).startsWith(normalize(context.convexRoot) + "/"),
    )
    .filter((sf) => !normalize(sf.fileName).includes("/_generated/"))
    .filter((sf) => !normalize(sf.fileName).endsWith("/schema.ts"));

  const items: FunctionEntry[] = [];
  for (const sourceFile of files) {
    sourceFile.forEachChild((node) => {
      if (!ts.isVariableStatement(node) || !hasExport(node)) return;
      for (const declaration of node.declarationList.declarations) {
        if (!ts.isIdentifier(declaration.name) || !declaration.initializer) continue;
        const init = deref(context, declaration.initializer);
        if (!init || !ts.isCallExpression(init)) continue;
        const fnKind = FUNCTION_KIND_MAP[expressionName(init.expression)];
        if (!fnKind) continue;
        const objectArg = deref(context, init.arguments[0]);
        if (!objectArg || !ts.isObjectLiteralExpression(objectArg)) continue;
        const argsProp = findProp(objectArg, "args");
        const returnsProp = findProp(objectArg, "returns");

        if (!argsProp) {
          pushDiagnostic(context, {
            code: "RX010",
            severity: "warning",
            message: `Function ${declaration.name.text} has no args validator`,
            symbol: declaration.name.text,
            provenance: "source",
            suggestion:
              "Add an args validator to enable request contract generation.",
            primary_span: origin(sourceFile, declaration),
            related_spans: [],
          });
        }

        if (!returnsProp) {
          pushDiagnostic(context, {
            code: "RX021",
            severity: "warning",
            message: `Function ${declaration.name.text} has no returns validator; response contract is lossy`,
            symbol: declaration.name.text,
            provenance: "source",
            suggestion:
              "Add a returns validator to enable strong response contract generation.",
            primary_span: origin(sourceFile, declaration),
            related_spans: [],
          });
        }

        items.push({
          canonical_path: `${path.basename(sourceFile.fileName, ".ts")}:${declaration.name.text}`,
          module_path: normalize(
            path.relative(context.convexRoot, sourceFile.fileName),
          ).replace(/\.ts$/, ""),
          export_name: declaration.name.text,
          visibility: fnKind.visibility,
          kind: fnKind.kind,
          args_type: argsProp
            ? parseArgsValue(context, argsProp.initializer, sourceFile)
            : null,
          returns_type: returnsProp
            ? parseValidator(context, returnsProp.initializer, sourceFile)
            : null,
          contract_provenance: returnsProp ? "validator" : "missing",
          source: origin(sourceFile, declaration),
        });
      }
    });
  }
  return items.sort((left, right) =>
    left.canonical_path.localeCompare(right.canonical_path),
  );
}

function parseArgsValue(
  context: AnalyzerContext,
  expression: ts.Expression,
  sourceFile: ts.SourceFile,
): TypeNode {
  const value = deref(context, expression);
  if (value && ts.isObjectLiteralExpression(value)) {
    const fields: Field[] = [];
    for (const prop of value.properties) {
      if (!ts.isPropertyAssignment(prop)) continue;
      const name = propertyName(prop.name);
      const validator = deref(context, prop.initializer);
      const parsed = parseValidator(context, validator, sourceFile);
      fields.push({
        name,
        required: !isOptionalValidator(context, validator),
        type: unwrapOptional(parsed),
        doc: null,
        source: origin(sourceFile, prop),
      });
    }
    return { kind: "object", fields, open: false };
  }
  return parseValidator(context, value, sourceFile);
}

function parseValidator(
  context: AnalyzerContext,
  expression: ts.Expression | undefined,
  sourceFile: ts.SourceFile,
): TypeNode {
  const expr = deref(context, expression);
  if (!expr) {
    return unknown("missing expression");
  }
  if (ts.isCallExpression(expr)) {
    const callee = expressionName(expr.expression);
    switch (callee) {
      case "v.string":
        return { kind: "string" };
      case "v.number":
        return { kind: "float64" };
      case "v.int64":
        return { kind: "int64" };
      case "v.boolean":
        return { kind: "boolean" };
      case "v.null":
        return { kind: "null" };
      case "v.bytes":
        return { kind: "bytes" };
      case "v.any":
        return { kind: "any" };
      case "v.literal":
        return parseLiteral(expr.arguments[0]);
      case "v.id": {
        const argument = expr.arguments[0];
        if (argument && ts.isStringLiteralLike(argument)) {
          return { kind: "id", table: argument.text };
        }
        pushDiagnostic(context, {
          code: "RX001",
          severity: "error",
          message:
            "Dynamic validator argument to v.id(...) is not statically analyzable",
          symbol: null,
          provenance: "source",
          suggestion: "Use a string literal table name in v.id(...).",
          primary_span: origin(sourceFile, expr),
          related_spans: [],
        });
        return unknown("dynamic_validator");
      }
      case "v.array":
        return {
          kind: "array",
          element: parseValidator(context, expr.arguments[0], sourceFile),
        };
      case "v.record":
        return {
          kind: "record",
          value: parseValidator(context, expr.arguments[1], sourceFile),
        };
      case "v.object":
        return parseObjectValidator(context, expr.arguments[0], sourceFile);
      case "v.union":
        return {
          kind: "union",
          members: expr.arguments.map((arg) =>
            parseValidator(context, arg, sourceFile),
          ),
        };
      case "v.optional":
        return {
          kind: "union",
          members: [
            parseValidator(context, expr.arguments[0], sourceFile),
            { kind: "null" },
          ],
        };
      default:
        return unknown(`unsupported call ${callee}`);
    }
  }
  if (ts.isObjectLiteralExpression(expr)) {
    return parseObjectValidator(context, expr, sourceFile);
  }
  return unknown(`unsupported expression ${ts.SyntaxKind[expr.kind]}`);
}

function parseObjectValidator(
  context: AnalyzerContext,
  expression: ts.Expression | undefined,
  sourceFile: ts.SourceFile,
): TypeNode {
  const objectLiteral = deref(context, expression);
  if (!objectLiteral || !ts.isObjectLiteralExpression(objectLiteral)) {
    return unknown("expected object literal");
  }

  const fields: Field[] = [];
  for (const prop of objectLiteral.properties) {
    if (ts.isPropertyAssignment(prop)) {
      const name = propertyName(prop.name);
      const validator = deref(context, prop.initializer);
      fields.push({
        name,
        required: !isOptionalValidator(context, validator),
        type: unwrapOptional(parseValidator(context, validator, sourceFile)),
        doc: null,
        source: origin(sourceFile, prop),
      });
      continue;
    }

    if (ts.isShorthandPropertyAssignment(prop)) {
      const resolved = resolveIdentifierValue(context, prop.name);
      if (resolved && ts.isObjectLiteralExpression(resolved)) {
        const nested = parseObjectValidator(context, resolved, sourceFile);
        if (nested.kind === "object") {
          fields.push(...nested.fields);
        }
      }
      continue;
    }

    if (ts.isSpreadAssignment(prop)) {
      const resolved = deref(context, prop.expression);
      if (resolved && ts.isObjectLiteralExpression(resolved)) {
        const nested = parseObjectValidator(context, resolved, sourceFile);
        if (nested.kind === "object") {
          fields.push(...nested.fields);
        }
      }
    }
  }

  return { kind: "object", fields, open: false };
}

function parseLiteral(expression: ts.Expression | undefined): TypeNode {
  if (!expression) return unknown("missing literal");
  if (ts.isStringLiteralLike(expression)) {
    return { kind: "literal_string", value: expression.text };
  }
  if (ts.isNumericLiteral(expression)) {
    return { kind: "literal_number", value: Number(expression.text) };
  }
  if (
    expression.kind === ts.SyntaxKind.TrueKeyword ||
    expression.kind === ts.SyntaxKind.FalseKeyword
  ) {
    return {
      kind: "literal_boolean",
      value: expression.kind === ts.SyntaxKind.TrueKeyword,
    };
  }
  return unknown("unsupported literal");
}

function unwrapOptional(type: TypeNode): TypeNode {
  if (
    type.kind === "union" &&
    type.members.length === 2 &&
    type.members.some((member) => member.kind === "null")
  ) {
    return type.members.find((member) => member.kind !== "null") ?? type;
  }
  return type;
}

function isOptionalValidator(
  context: AnalyzerContext,
  expression: ts.Expression | undefined,
): boolean {
  const expr = deref(context, expression);
  return Boolean(
    expr && ts.isCallExpression(expr) && expressionName(expr.expression) === "v.optional",
  );
}

function deref(
  context: AnalyzerContext,
  expression: ts.Expression | undefined,
): ts.Expression | undefined {
  if (!expression) return undefined;
  if (
    ts.isParenthesizedExpression(expression) ||
    ts.isAsExpression(expression) ||
    ts.isSatisfiesExpression(expression)
  ) {
    return deref(context, expression.expression);
  }
  if (ts.isIdentifier(expression)) {
    return resolveIdentifierValue(context, expression) ?? expression;
  }
  return expression;
}

function resolveIdentifierValue(
  context: AnalyzerContext,
  identifier: ts.Identifier,
): ts.Expression | undefined {
  const symbol = context.checker.getSymbolAtLocation(identifier);
  if (!symbol) return undefined;
  for (const decl of symbol.declarations ?? []) {
    if (ts.isVariableDeclaration(decl) && decl.initializer) {
      return deref(context, decl.initializer);
    }
    if (ts.isPropertyAssignment(decl)) {
      return deref(context, decl.initializer);
    }
  }
  return undefined;
}

function expressionName(expression: ts.Expression): string {
  if (ts.isIdentifier(expression)) return expression.text;
  if (ts.isPropertyAccessExpression(expression)) {
    return `${expressionName(expression.expression)}.${expression.name.text}`;
  }
  return "";
}

function findProp(
  objectLiteral: ts.ObjectLiteralExpression,
  name: string,
): ts.PropertyAssignment | undefined {
  return objectLiteral.properties.find(
    (prop): prop is ts.PropertyAssignment =>
      ts.isPropertyAssignment(prop) && propertyName(prop.name) === name,
  );
}

function propertyName(name: ts.PropertyName): string {
  if (ts.isIdentifier(name) || ts.isStringLiteralLike(name)) return name.text;
  return name.getText();
}

function hasExport(node: ts.VariableStatement): boolean {
  return Boolean(
    node.modifiers?.some(
      (modifier) => modifier.kind === ts.SyntaxKind.ExportKeyword,
    ),
  );
}

function origin(sourceFile: ts.SourceFile, node: ts.Node): Origin {
  const pos = sourceFile.getLineAndCharacterOfPosition(node.getStart(sourceFile));
  return {
    file: normalize(sourceFile.fileName),
    line: pos.line + 1,
    column: pos.character + 1,
  };
}

function unknown(reason: string): TypeNode {
  return { kind: "unknown", reason, confidence: 0 };
}

function pascalCase(input: string): string {
  return input
    .split(/[^a-zA-Z0-9]/)
    .filter(Boolean)
    .map((part) => part[0]?.toUpperCase() + part.slice(1))
    .join("");
}

function pushDiagnostic(context: AnalyzerContext, diagnostic: Diagnostic): void {
  context.diagnostics.push(diagnostic);
}
