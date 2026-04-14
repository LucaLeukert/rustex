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
  snippet: string | null;
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
  component_path: string | null;
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
    discovered_convex_roots: string[];
    component_roots: string[];
  };
  tables: Table[];
  functions: FunctionEntry[];
  named_types: unknown[];
  constraints: unknown[];
  capabilities: {
    generated_metadata_present: boolean;
    inferred_returns_used: boolean;
    internal_functions_present: boolean;
    public_functions_present: boolean;
    http_actions_present: boolean;
    components_present: boolean;
  };
  source_inventory: Array<{ path: string; kind: string }>;
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
  readonly allowInferredReturns: boolean;
};

const FUNCTION_KIND_MAP: Record<
  string,
  { kind: FunctionKind; visibility: Visibility }
> = {
  query: { kind: "query", visibility: "public" },
  mutation: { kind: "mutation", visibility: "public" },
  action: { kind: "action", visibility: "public" },
  httpAction: { kind: "action", visibility: "public" },
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
    allowInferredReturns: args.allowInferredReturns,
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
      discovered_convex_roots: [normalize(args.convexRoot)],
      component_roots: discoverComponentRoots(args.convexRoot),
    },
    tables,
    functions: extractFunctions(context),
    named_types: [],
    constraints: [],
    capabilities: {
      generated_metadata_present: fs.existsSync(
        path.join(args.convexRoot, "_generated", "api.d.ts"),
      ),
      inferred_returns_used: false,
      internal_functions_present: false,
      public_functions_present: false,
      http_actions_present: false,
      components_present: false,
    },
    source_inventory: buildSourceInventory(args.convexRoot),
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
  const allowInferredReturns = argv.includes("--allow-inferred-returns");
  const filteredArgv = argv.filter((arg) => arg !== "--allow-inferred-returns");
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
    Options.processCommandLine(parser, filteredArgv, CliConfig.defaultConfig),
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
        allowInferredReturns,
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
        if (!objectArg || !ts.isObjectLiteralExpression(objectArg)) {
          if (expressionName(init.expression) !== "httpAction") continue;
        }
        const handler =
          objectArg && ts.isObjectLiteralExpression(objectArg)
            ? findFunctionProp(objectArg, "handler")
            : init.arguments[0];
        const argsProp =
          objectArg && ts.isObjectLiteralExpression(objectArg)
            ? findProp(objectArg, "args")
            : undefined;
        const returnsProp =
          objectArg && ts.isObjectLiteralExpression(objectArg)
            ? findProp(objectArg, "returns")
            : undefined;

        if (!argsProp && expressionName(init.expression) !== "httpAction") {
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
            snippet: snippet(sourceFile, declaration),
          });
        }

        if (!returnsProp) {
          const canInfer = context.allowInferredReturns && handler;
          pushDiagnostic(context, {
            code: canInfer ? "RX022" : "RX021",
            severity: "warning",
            message: canInfer
              ? `Function ${declaration.name.text} has no returns validator; inferring response contract from the TypeScript checker`
              : `Function ${declaration.name.text} has no returns validator; response contract is lossy`,
            symbol: declaration.name.text,
            provenance: canInfer ? "inferred" : "source",
            suggestion: canInfer
              ? "Add a returns validator to make the inferred contract explicit and stable."
              : "Add a returns validator to enable strong response contract generation.",
            primary_span: origin(sourceFile, declaration),
            related_spans: [],
            snippet: snippet(sourceFile, declaration),
          });
        }

        const inferredReturns =
          !returnsProp && context.allowInferredReturns && handler
            ? inferHandlerReturnType(context, handler, sourceFile)
            : null;

        items.push({
          canonical_path: `${path.basename(sourceFile.fileName, ".ts")}:${declaration.name.text}`,
          module_path: normalize(
            path.relative(context.convexRoot, sourceFile.fileName),
          ).replace(/\.ts$/, ""),
          export_name: declaration.name.text,
          component_path: componentPathForModule(
            normalize(path.relative(context.convexRoot, sourceFile.fileName)).replace(/\.ts$/, ""),
          ),
          visibility: fnKind.visibility,
          kind: fnKind.kind,
          args_type: argsProp
            ? parseArgsValue(context, argsProp.initializer, sourceFile)
            : null,
          returns_type: returnsProp
            ? parseValidator(context, returnsProp.initializer, sourceFile)
            : inferredReturns,
          contract_provenance: returnsProp
            ? "validator"
            : inferredReturns
              ? "inferred"
              : "missing",
          source: origin(sourceFile, declaration),
        });
      }
    });
  }
  reconcileGeneratedApiTopology(context, items);
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
          snippet: snippet(sourceFile, expr),
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
        pushDiagnostic(context, {
          code: "RX040",
          severity: "warning",
          message: `Unsupported validator helper ${callee}; falling back to an unknown contract`,
          symbol: null,
          provenance: "source",
          suggestion:
            "Inline the validator expression or export a statically analyzable object/validator literal.",
          primary_span: origin(sourceFile, expr),
          related_spans: [],
          snippet: snippet(sourceFile, expr),
        });
        return unknown(`unsupported call ${callee}`);
    }
  }
  if (ts.isObjectLiteralExpression(expr)) {
    return parseObjectValidator(context, expr, sourceFile);
  }
  if (ts.isIdentifier(expr)) {
    pushDiagnostic(context, {
      code: "RX041",
      severity: "warning",
      message: `Opaque helper ${expr.text} could not be resolved statically`,
      symbol: expr.text,
      provenance: "source",
      suggestion:
        "Export a concrete validator value or inline the helper where the contract is declared.",
      primary_span: origin(sourceFile, expr),
      related_spans: [],
      snippet: snippet(sourceFile, expr),
    });
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
      } else {
        pushDiagnostic(context, {
          code: "RX042",
          severity: "warning",
          message: `Shorthand helper ${prop.name.text} did not resolve to a static object literal`,
          symbol: prop.name.text,
          provenance: "source",
          suggestion:
            "Expand the shorthand helper inline if you want Rustex to recover its fields.",
          primary_span: origin(sourceFile, prop),
          related_spans: [],
          snippet: snippet(sourceFile, prop),
        });
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
      } else {
        pushDiagnostic(context, {
          code: "RX043",
          severity: "warning",
          message: "Spread helper did not resolve to a static object literal",
          symbol: null,
          provenance: "source",
          suggestion:
            "Replace the spread with a literal object shape or an imported constant that resolves to one.",
          primary_span: origin(sourceFile, prop),
          related_spans: [],
          snippet: snippet(sourceFile, prop),
        });
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

function findFunctionProp(
  objectLiteral: ts.ObjectLiteralExpression,
  name: string,
): ts.Expression | undefined {
  const prop = findProp(objectLiteral, name);
  return prop?.initializer;
}

function componentPathForModule(modulePath: string): string | null {
  if (!modulePath.startsWith("components/")) {
    return null;
  }
  const parts = modulePath.split("/");
  return parts.length >= 3 ? parts.slice(0, 2).join("/") : "components";
}

function discoverComponentRoots(convexRoot: string): string[] {
  const componentsRoot = path.join(convexRoot, "components");
  if (!fs.existsSync(componentsRoot) || !fs.statSync(componentsRoot).isDirectory()) {
    return [];
  }
  return fs
    .readdirSync(componentsRoot, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => normalize(path.join(componentsRoot, entry.name)))
    .sort();
}

function buildSourceInventory(convexRoot: string): Array<{ path: string; kind: string }> {
  const items: Array<{ path: string; kind: string }> = [];
  walk(convexRoot, (file) => {
    const normalized = normalize(file);
    if (normalized.endsWith("/schema.ts")) {
      items.push({ path: normalized, kind: "schema" });
    } else if (normalized.includes("/_generated/")) {
      items.push({ path: normalized, kind: "generated_metadata" });
    } else if (normalized.includes("/components/")) {
      items.push({ path: normalized, kind: "component_module" });
    } else if (normalized.endsWith(".ts") || normalized.endsWith(".tsx")) {
      items.push({ path: normalized, kind: "function_module" });
    }
  });
  return items.sort((left, right) => left.path.localeCompare(right.path));
}

function inferHandlerReturnType(
  context: AnalyzerContext,
  handler: ts.Expression,
  sourceFile: ts.SourceFile,
): TypeNode | null {
  const resolved = deref(context, handler) ?? handler;
  if (
    !ts.isArrowFunction(resolved) &&
    !ts.isFunctionExpression(resolved) &&
    !ts.isMethodDeclaration(resolved)
  ) {
    pushDiagnostic(context, {
      code: "RX050",
      severity: "warning",
      message: "Return inference skipped because the handler is not a function expression",
      symbol: null,
      provenance: "inferred",
      suggestion:
        "Keep the handler inline or add a returns validator to make the response contract explicit.",
      primary_span: origin(sourceFile, resolved),
      related_spans: [],
      snippet: snippet(sourceFile, resolved),
    });
    return null;
  }

  const syntaxReturn = inferHandlerReturnFromSyntax(context, resolved, sourceFile);
  if (syntaxReturn && !matchesUnknownOrAny(syntaxReturn)) {
    return syntaxReturn;
  }

  const signature = context.checker.getSignatureFromDeclaration(resolved);
  const rawType = signature
    ? context.checker.getReturnTypeOfSignature(signature)
    : context.checker.getTypeAtLocation(resolved);
  const checkerWithPromiseHelpers = context.checker as ts.TypeChecker & {
    getPromisedTypeOfPromise?(type: ts.Type): ts.Type | undefined;
    getAwaitedType?(type: ts.Type): ts.Type | undefined;
  };
  const awaited =
    checkerWithPromiseHelpers.getAwaitedType?.(rawType) ??
    checkerWithPromiseHelpers.getPromisedTypeOfPromise?.(rawType) ??
    rawType;
  return typeToNode(context, awaited, sourceFile, new Set());
}

function inferHandlerReturnFromSyntax(
  context: AnalyzerContext,
  handler: ts.ArrowFunction | ts.FunctionExpression | ts.MethodDeclaration,
  sourceFile: ts.SourceFile,
): TypeNode | null {
  if (!handler.body) {
    return null;
  }
  if (ts.isExpression(handler.body)) {
    return inferTypeFromExpressionSyntax(context, handler.body, sourceFile);
  }

  const returns: TypeNode[] = [];
  const visit = (node: ts.Node) => {
    if (ts.isReturnStatement(node) && node.expression) {
      const inferred = inferTypeFromExpressionSyntax(context, node.expression, sourceFile);
      if (inferred) {
        returns.push(inferred);
      }
    }
    if (
      ts.isFunctionDeclaration(node) ||
      ts.isFunctionExpression(node) ||
      ts.isArrowFunction(node) ||
      ts.isMethodDeclaration(node)
    ) {
      return;
    }
    node.forEachChild(visit);
  };
  handler.body.forEachChild(visit);

  if (returns.length === 0) {
    return null;
  }
  if (returns.length === 1) {
    return returns[0] ?? null;
  }
  return { kind: "union", members: returns };
}

function typeToNode(
  context: AnalyzerContext,
  type: ts.Type,
  sourceFile: ts.SourceFile,
  seen: Set<ts.Type>,
): TypeNode {
  if (seen.has(type)) {
    return unknown("recursive_type");
  }
  seen.add(type);

  const renderedType = context.checker.typeToString(type);

  if (type.flags & ts.TypeFlags.StringLike) return { kind: "string" };
  if (type.flags & ts.TypeFlags.NumberLike) return { kind: "float64" };
  if (type.flags & ts.TypeFlags.BigIntLike) return { kind: "int64" };
  if (type.flags & ts.TypeFlags.BooleanLike) return { kind: "boolean" };
  if (type.flags & ts.TypeFlags.Null) return { kind: "null" };
  if (type.flags & ts.TypeFlags.Any) {
    if (renderedType === "number") return { kind: "float64" };
    if (renderedType === "bigint") return { kind: "int64" };
    if (renderedType === "string") return { kind: "string" };
    if (renderedType === "boolean") return { kind: "boolean" };
    return { kind: "any" };
  }
  if (type.flags & ts.TypeFlags.Unknown) return unknown("unknown_type");
  if (type.flags & ts.TypeFlags.StringLiteral) {
    return {
      kind: "literal_string",
      value: (type as ts.StringLiteralType).value,
    };
  }
  if (type.flags & ts.TypeFlags.NumberLiteral) {
    return {
      kind: "literal_number",
      value: Number((type as ts.NumberLiteralType).value),
    };
  }
  if (type.flags & ts.TypeFlags.BooleanLiteral) {
    return {
      kind: "literal_boolean",
      value: context.checker.typeToString(type) === "true",
    };
  }
  if (type.isUnion()) {
    return {
      kind: "union",
      members: type.types.map((member) =>
        typeToNode(context, member, sourceFile, new Set(seen)),
      ),
    };
  }

  if (context.checker.isArrayType(type)) {
    const element =
      context.checker.getTypeArguments(type as ts.TypeReference)[0] ??
      context.checker.getIndexTypeOfType(type, ts.IndexKind.Number);
    return {
      kind: "array",
      element: typeToNode(
        context,
        element ?? context.checker.getAnyType(),
        sourceFile,
        new Set(seen),
      ),
    };
  }

  const typeName = renderedType;
  if (typeName === "ArrayBuffer" || typeName === "Uint8Array") {
    return { kind: "bytes" };
  }

  const symbol = type.getSymbol();
  if (symbol?.getName() === "GenericId" || typeName.startsWith("Id<")) {
    const match = typeName.match(/<"([^"]+)">/);
    const table = match?.[1];
    if (table) {
      return { kind: "id", table };
    }
  }

  const properties = context.checker.getPropertiesOfType(type);
  if (properties.length > 0) {
    const fields = properties
      .filter((prop) => prop.getName() !== "_id" || true)
      .map((prop) => {
        const propertyDeclaration =
          [prop.valueDeclaration, ...(prop.declarations ?? [])].find((decl): decl is ts.PropertyAssignment =>
            Boolean(decl && ts.isPropertyAssignment(decl)),
          );
        const declaration =
          propertyDeclaration ?? prop.valueDeclaration ?? prop.declarations?.[0] ?? sourceFile;
        const syntaxType = propertyDeclaration
          ? inferTypeFromExpressionSyntax(context, propertyDeclaration.initializer, sourceFile)
          : null;
        const valueType = propertyDeclaration
          ? context.checker.getTypeAtLocation(propertyDeclaration.initializer)
          : context.checker.getTypeOfSymbolAtLocation(prop, declaration);
        const optional = (prop.flags & ts.SymbolFlags.Optional) !== 0;
        const inferredType =
          syntaxType && !matchesUnknownOrAny(syntaxType)
            ? syntaxType
            : unwrapOptional(
                typeToNode(context, valueType, sourceFile, new Set(seen)),
              );
        return {
          name: prop.getName(),
          required: !optional,
          type: inferredType,
          doc: ts.displayPartsToString(prop.getDocumentationComment(context.checker)) || null,
          source: declaration ? origin(sourceFile, declaration) : null,
        } satisfies Field;
      });
    return { kind: "object", fields, open: false };
  }

  pushDiagnostic(context, {
    code: "RX051",
    severity: "note",
    message: `TypeScript return inference fell back to an unknown contract for ${typeName}`,
    symbol: null,
    provenance: "inferred",
    suggestion:
      "Add a returns validator if you need a stronger generated response contract.",
    primary_span: null,
    related_spans: [],
    snippet: null,
  });
  return unknown(`type_inference:${typeName}`);
}

function reconcileGeneratedApiTopology(
  context: AnalyzerContext,
  functions: FunctionEntry[],
): void {
  const generatedApiPath = path.join(context.convexRoot, "_generated", "api.d.ts");
  if (!fs.existsSync(generatedApiPath)) {
    return;
  }

  const sourceFile = context.program
    .getSourceFiles()
    .find((sf) => normalize(sf.fileName) === normalize(generatedApiPath));
  if (!sourceFile) {
    return;
  }

  const generatedModules = sourceFile.statements
    .filter(ts.isImportDeclaration)
    .map((statement) => statement.moduleSpecifier)
    .filter(ts.isStringLiteralLike)
    .map((specifier) => normalize(specifier.text).replace(/^\.\.\//, "").replace(/\.js$/, ""))
    .filter((modulePath) => modulePath !== "_generated/server" && modulePath !== "_generated/api")
    .sort();

  const extractedModules = Array.from(
    new Set(
      functions
        .map((fn) => fn.module_path)
        .filter((modulePath) => modulePath !== "http"),
    ),
  ).sort();

  for (const modulePath of extractedModules) {
    if (!generatedModules.includes(modulePath)) {
      pushDiagnostic(context, {
        code: "RX060",
        severity: "warning",
        message: `Generated Convex API metadata is missing module ${modulePath}`,
        symbol: modulePath,
        provenance: "generated_ts",
        suggestion:
          "Run `npx convex dev` or `npx convex codegen` to refresh convex/_generated metadata.",
        primary_span: origin(sourceFile, sourceFile),
        related_spans: [],
        snippet: snippet(sourceFile, sourceFile),
      });
    }
  }

  for (const modulePath of generatedModules) {
    if (!extractedModules.includes(modulePath)) {
      pushDiagnostic(context, {
        code: "RX061",
        severity: "note",
        message: `Generated Convex API metadata references module ${modulePath}, but Rustex did not extract any functions from it`,
        symbol: modulePath,
        provenance: "generated_ts",
        suggestion:
          "Check for unsupported component or helper patterns in that module, or remove stale generated metadata.",
        primary_span: origin(sourceFile, sourceFile),
        related_spans: [],
        snippet: snippet(sourceFile, sourceFile),
      });
    }
  }
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

function inferTypeFromExpressionSyntax(
  context: AnalyzerContext,
  expression: ts.Expression,
  sourceFile: ts.SourceFile,
): TypeNode | null {
  const resolved = deref(context, expression) ?? expression;

  if (ts.isParenthesizedExpression(resolved) || ts.isAsExpression(resolved) || ts.isSatisfiesExpression(resolved)) {
    return inferTypeFromExpressionSyntax(context, resolved.expression, sourceFile);
  }
  if (ts.isStringLiteralLike(resolved)) return { kind: "string" };
  if (ts.isNumericLiteral(resolved)) return { kind: "float64" };
  if (
    resolved.kind === ts.SyntaxKind.TrueKeyword ||
    resolved.kind === ts.SyntaxKind.FalseKeyword
  ) {
    return { kind: "boolean" };
  }
  if (ts.isObjectLiteralExpression(resolved)) {
    const fields = resolved.properties.flatMap((prop) => {
      if (!ts.isPropertyAssignment(prop)) return [];
      const name = propertyName(prop.name);
      return [{
        name,
        required: true,
        type: inferTypeFromExpressionSyntax(context, prop.initializer, sourceFile) ?? unknown("expression_property"),
        doc: null,
        source: origin(sourceFile, prop),
      } satisfies Field];
    });
    return { kind: "object", fields, open: false };
  }
  if (ts.isCallExpression(resolved)) {
    const callee = expressionName(resolved.expression);
    if (callee === "Date.now") {
      return { kind: "float64" };
    }
  }
  if (ts.isArrayLiteralExpression(resolved)) {
    const members = resolved.elements
      .filter(ts.isExpression)
      .map((element) => inferTypeFromExpressionSyntax(context, element, sourceFile))
      .filter((node): node is TypeNode => node !== null);
    const element =
      members.length === 0
        ? { kind: "any" as const }
        : members.length === 1
          ? (members[0] ?? { kind: "any" as const })
          : { kind: "union" as const, members };
    return { kind: "array", element };
  }
  if (ts.isPropertyAccessExpression(resolved) && resolved.name.text === "length") {
    return { kind: "float64" };
  }
  if (ts.isIdentifier(resolved) || ts.isPropertyAccessExpression(resolved) || ts.isElementAccessExpression(resolved)) {
    const checkerType = context.checker.getTypeAtLocation(resolved);
    const fromChecker = typeToNode(context, checkerType, sourceFile, new Set());
    if (!matchesUnknownOrAny(fromChecker)) {
      return fromChecker;
    }
  }
  if (ts.isAwaitExpression(resolved)) {
    return inferTypeFromExpressionSyntax(context, resolved.expression, sourceFile);
  }
  if (ts.isConditionalExpression(resolved)) {
    const whenTrue = inferTypeFromExpressionSyntax(context, resolved.whenTrue, sourceFile);
    const whenFalse = inferTypeFromExpressionSyntax(context, resolved.whenFalse, sourceFile);
    if (whenTrue && whenFalse) {
      return { kind: "union", members: [whenTrue, whenFalse] };
    }
  }
  if (ts.isBinaryExpression(resolved)) {
    if (
      resolved.operatorToken.kind === ts.SyntaxKind.PlusToken ||
      resolved.operatorToken.kind === ts.SyntaxKind.MinusToken ||
      resolved.operatorToken.kind === ts.SyntaxKind.AsteriskToken ||
      resolved.operatorToken.kind === ts.SyntaxKind.SlashToken
    ) {
      return { kind: "float64" };
    }
    if (
      resolved.operatorToken.kind === ts.SyntaxKind.EqualsEqualsEqualsToken ||
      resolved.operatorToken.kind === ts.SyntaxKind.EqualsEqualsToken ||
      resolved.operatorToken.kind === ts.SyntaxKind.ExclamationEqualsEqualsToken ||
      resolved.operatorToken.kind === ts.SyntaxKind.ExclamationEqualsToken
    ) {
      return { kind: "boolean" };
    }
  }

  return null;
}

function matchesUnknownOrAny(node: TypeNode): boolean {
  return node.kind === "any" || node.kind === "unknown";
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

function snippet(sourceFile: ts.SourceFile, node: ts.Node): string {
  const start = sourceFile.getLineAndCharacterOfPosition(node.getStart(sourceFile)).line;
  const end = sourceFile.getLineAndCharacterOfPosition(node.getEnd()).line;
  return sourceFile.text
    .split(/\r?\n/)
    .slice(start, Math.min(end + 1, start + 3))
    .join("\n");
}
