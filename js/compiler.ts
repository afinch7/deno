// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/cli/msg_generated";
import { core } from "./core";
import * as flatbuffers from "./flatbuffers";
import { sendSync } from "./dispatch";
import { TextDecoder } from "./text_encoding";
import * as ts from "typescript";
import * as os from "./os";
import { bold, cyan, yellow } from "./colors";
import { window } from "./window";
import { postMessage, workerClose, workerMain } from "./workers";
import { Console } from "./console";
import { assert, notImplemented } from "./util";
import * as util from "./util";
import { cwd } from "./dir";
import { assetSourceCode } from "./assets";

// Startup boilerplate. This is necessary because the compiler has its own
// snapshot. (It would be great if we could remove these things or centralize
// them somewhere else.)
const console = new Console(core.print);
window.console = console;
window.workerMain = workerMain;
export default function denoMain(): void {
  os.start("TS");
}

const ASSETS = "$asset$";
const OUT_DIR = "$deno$";

/** The format of the work message payload coming from the privileged side */
interface CompilerReq {
  rootNames: string[];
  // TODO(ry) add compiler config to this interface.
  // options: ts.CompilerOptions;
  configPath?: string;
  config?: string;
}

/** Options that either do nothing in Deno, or would cause undesired behavior
 * if modified. */
const ignoredCompilerOptions: ReadonlyArray<string> = [
  "allowSyntheticDefaultImports",
  "baseUrl",
  "build",
  "composite",
  "declaration",
  "declarationDir",
  "declarationMap",
  "diagnostics",
  "downlevelIteration",
  "emitBOM",
  "emitDeclarationOnly",
  "esModuleInterop",
  "extendedDiagnostics",
  "forceConsistentCasingInFileNames",
  "help",
  "importHelpers",
  "incremental",
  "inlineSourceMap",
  "inlineSources",
  "init",
  "isolatedModules",
  "lib",
  "listEmittedFiles",
  "listFiles",
  "mapRoot",
  "maxNodeModuleJsDepth",
  "module",
  "moduleResolution",
  "newLine",
  "noEmit",
  "noEmitHelpers",
  "noEmitOnError",
  "noLib",
  "noResolve",
  "out",
  "outDir",
  "outFile",
  "paths",
  "preserveSymlinks",
  "preserveWatchOutput",
  "pretty",
  "rootDir",
  "rootDirs",
  "showConfig",
  "skipDefaultLibCheck",
  "skipLibCheck",
  "sourceMap",
  "sourceRoot",
  "stripInternal",
  "target",
  "traceResolution",
  "tsBuildInfoFile",
  "types",
  "typeRoots",
  "version",
  "watch"
];

interface ModuleMetaData {
  moduleName: string | undefined;
  filename: string | undefined;
  mediaType: msg.MediaType;
  sourceCode: string | undefined;
}

function fetchModuleMetaData(
  specifier: string,
  referrer: string
): ModuleMetaData {
  util.log("compiler.fetchModuleMetaData", { specifier, referrer });
  // Send FetchModuleMetaData message
  const builder = flatbuffers.createBuilder();
  const specifier_ = builder.createString(specifier);
  const referrer_ = builder.createString(referrer);
  const inner = msg.FetchModuleMetaData.createFetchModuleMetaData(
    builder,
    specifier_,
    referrer_
  );
  const baseRes = sendSync(builder, msg.Any.FetchModuleMetaData, inner);
  assert(baseRes != null);
  assert(
    msg.Any.FetchModuleMetaDataRes === baseRes!.innerType(),
    `base.innerType() unexpectedly is ${baseRes!.innerType()}`
  );
  const fetchModuleMetaDataRes = new msg.FetchModuleMetaDataRes();
  assert(baseRes!.inner(fetchModuleMetaDataRes) != null);
  const dataArray = fetchModuleMetaDataRes.dataArray();
  const decoder = new TextDecoder();
  const sourceCode = dataArray ? decoder.decode(dataArray) : undefined;
  // flatbuffers returns `null` for an empty value, this does not fit well with
  // idiomatic TypeScript under strict null checks, so converting to `undefined`
  return {
    moduleName: fetchModuleMetaDataRes.moduleName() || undefined,
    filename: fetchModuleMetaDataRes.filename() || undefined,
    mediaType: fetchModuleMetaDataRes.mediaType(),
    sourceCode
  };
}

/** For caching source map and compiled js */
function cache(extension: string, moduleId: string, contents: string): void {
  util.log("compiler.cache", moduleId);
  const builder = flatbuffers.createBuilder();
  const extension_ = builder.createString(extension);
  const moduleId_ = builder.createString(moduleId);
  const contents_ = builder.createString(contents);
  const inner = msg.Cache.createCache(
    builder,
    extension_,
    moduleId_,
    contents_
  );
  const baseRes = sendSync(builder, msg.Any.Cache, inner);
  assert(baseRes == null);
}

/** Returns the TypeScript Extension enum for a given media type. */
function getExtension(
  fileName: string,
  mediaType: msg.MediaType
): ts.Extension {
  switch (mediaType) {
    case msg.MediaType.JavaScript:
      return ts.Extension.Js;
    case msg.MediaType.TypeScript:
      return fileName.endsWith(".d.ts") ? ts.Extension.Dts : ts.Extension.Ts;
    case msg.MediaType.Json:
      return ts.Extension.Json;
    case msg.MediaType.Dylib:
      return ts.Extension.Ts;
    case msg.MediaType.Unknown:
    default:
      throw TypeError("Cannot resolve extension.");
  }
}

class Host implements ts.CompilerHost {
  private readonly _options: ts.CompilerOptions = {
    allowJs: true,
    allowNonTsExtensions: true,
    checkJs: false,
    esModuleInterop: true,
    module: ts.ModuleKind.ESNext,
    outDir: OUT_DIR,
    resolveJsonModule: true,
    sourceMap: true,
    stripComments: true,
    target: ts.ScriptTarget.ESNext
  };
<<<<<<< HEAD
  // A reference to the `./os.ts` module, so it can be monkey patched during
  // testing
  private _os: Os = os;
  // Used to contain the script file we are currently running
  private _scriptFileNames: string[] = [];
  // A reference to the TypeScript LanguageService instance so it can be
  // monkey patched during testing
  private _service: ts.LanguageService;
  // A reference to `typescript` module so it can be monkey patched during
  // testing
  private _ts: Ts = ts;

  private readonly _assetsSourceCode: { [key: string]: string };

  /** The TypeScript language service often refers to the resolved fileName of
   * a module, this is a shortcut to avoid unnecessary module resolution logic
   * for modules that may have been initially resolved by a `moduleSpecifier`
   * and `containingFile`.  Also, `resolveModule()` throws when the module
   * cannot be resolved, which isn't always valid when dealing with the
   * TypeScript compiler, but the TypeScript compiler shouldn't be asking about
   * external modules that we haven't told it about yet.
   */
  private _getModuleMetaData(
    fileName: ModuleFileName
  ): ModuleMetaData | undefined {
    return (
      this._moduleMetaDataMap.get(fileName) ||
      (fileName.startsWith(ASSETS)
        ? this._resolveModule(fileName, "")
        : undefined)
    );
  }

  /** Log TypeScript diagnostics to the console and exit */
  private _logDiagnostics(diagnostics: ts.Diagnostic[]): never {
    const errMsg = this._os.noColor
      ? this._ts.formatDiagnostics(diagnostics, this)
      : this._ts.formatDiagnosticsWithColorAndContext(diagnostics, this);

    console.log(errMsg);
    // TODO The compiler isolate shouldn't exit.  Errors should be forwarded to
    // to the caller and the caller exit.
    return this._os.exit(1);
  }

  /** Given a `moduleSpecifier` and `containingFile` retrieve the cached
   * `fileName` for a given module.  If the module has yet to be resolved
   * this will return `undefined`.
   */
  private _resolveFileName(
    moduleSpecifier: ModuleSpecifier,
    containingFile: ContainingFile
  ): ModuleFileName | undefined {
    this._log("compiler._resolveFileName", { moduleSpecifier, containingFile });
    const innerMap = this._fileNamesMap.get(containingFile);
    if (innerMap) {
      return innerMap.get(moduleSpecifier);
    }
    return undefined;
  }

  /** Given a `moduleSpecifier` and `containingFile`, resolve the module and
   * return the `ModuleMetaData`.
   */
  private _resolveModule(
    moduleSpecifier: ModuleSpecifier,
    containingFile: ContainingFile
  ): ModuleMetaData {
    this._log("compiler._resolveModule", { moduleSpecifier, containingFile });
    assert(moduleSpecifier != null && moduleSpecifier.length > 0);
    let fileName = this._resolveFileName(moduleSpecifier, containingFile);
    if (fileName && this._moduleMetaDataMap.has(fileName)) {
      return this._moduleMetaDataMap.get(fileName)!;
    }
    let moduleId: ModuleId | undefined;
    let mediaType = msg.MediaType.Unknown;
    let sourceCode: SourceCode | undefined;
    if (
      moduleSpecifier.startsWith(ASSETS) ||
      containingFile.startsWith(ASSETS)
    ) {
      // Assets are compiled into the runtime javascript bundle.
      // we _know_ `.pop()` will return a string, but TypeScript doesn't so
      // not null assertion
      moduleId = moduleSpecifier.split("/").pop()!;
      const assetName = moduleId.includes(".") ? moduleId : `${moduleId}.d.ts`;
      assert(
        assetName in this._assetsSourceCode,
        `No such asset "${assetName}"`
      );
      mediaType = msg.MediaType.TypeScript;
      sourceCode = this._assetsSourceCode[assetName];
      fileName = `${ASSETS}/${assetName}`;
    } else {
      // We query Rust with a CodeFetch message. It will load the sourceCode,
      // and if there is any outputCode cached, will return that as well.
      const fetchResponse = this._os.fetchModuleMetaData(
        moduleSpecifier,
        containingFile
      );
      moduleId = fetchResponse.moduleName;
      fileName = fetchResponse.filename;
      mediaType = fetchResponse.mediaType;
      sourceCode = fetchResponse.sourceCode;
    }
    assert(moduleId != null, "No module ID.");
    assert(fileName != null, "No file name.");
    assert(
      mediaType !== msg.MediaType.Unknown,
      `Unknown media type for: "${moduleSpecifier}" from "${containingFile}".`
    );
    this._log(
      "resolveModule sourceCode length:",
      sourceCode && sourceCode.length
    );
    this._log("resolveModule has media type:", msg.MediaType[mediaType]);
    // fileName is asserted above, but TypeScript does not track so not null
    this._setFileName(moduleSpecifier, containingFile, fileName!);
    if (fileName && this._moduleMetaDataMap.has(fileName)) {
      return this._moduleMetaDataMap.get(fileName)!;
    }
    const moduleMetaData = new ModuleMetaData(
      moduleId!,
      fileName!,
      mediaType,
      sourceCode
    );
    this._moduleMetaDataMap.set(fileName!, moduleMetaData);
    return moduleMetaData;
  }

  /** Caches the resolved `fileName` in relationship to the `moduleSpecifier`
   * and `containingFile` in order to reduce calls to the privileged side
   * to retrieve the contents of a module.
   */
  private _setFileName(
    moduleSpecifier: ModuleSpecifier,
    containingFile: ContainingFile,
    fileName: ModuleFileName
  ): void {
    this._log("compiler._setFileName", { moduleSpecifier, containingFile });
    let innerMap = this._fileNamesMap.get(containingFile);
    if (!innerMap) {
      innerMap = new Map();
      this._fileNamesMap.set(containingFile, innerMap);
    }
    innerMap.set(moduleSpecifier, fileName);
  }

  constructor(assetsSourceCode: { [key: string]: string }) {
    this._assetsSourceCode = assetsSourceCode;
    this._service = this._ts.createLanguageService(this);
  }

  // Deno specific compiler API

  /** Retrieve the output of the TypeScript compiler for a given module.
   */
  compile(
    moduleSpecifier: ModuleSpecifier,
    containingFile: ContainingFile
  ): { outputCode: OutputCode; sourceMap: SourceMap } {
    this._log("compiler.compile", { moduleSpecifier, containingFile });
    const moduleMetaData = this._resolveModule(moduleSpecifier, containingFile);
    const { fileName, mediaType, sourceCode } = moduleMetaData;
    this._scriptFileNames = [fileName];
    let outputCode: string;
    let sourceMap = "";
    // Instead of using TypeScript to transpile JSON modules, we will just do
    // it directly.
    if (mediaType === msg.MediaType.Json) {
      outputCode = moduleMetaData.outputCode = jsonEsmTemplate(
        sourceCode,
        fileName
      );
    } else {
      const service = this._service;
      assert(
        mediaType === msg.MediaType.TypeScript ||
          mediaType === msg.MediaType.JavaScript ||
          mediaType === msg.MediaType.Dylib
      );
      const output = service.getEmitOutput(fileName);

      // Get the relevant diagnostics - this is 3x faster than
      // `getPreEmitDiagnostics`.
      const diagnostics = [
        // TypeScript is overly opinionated that only CommonJS modules kinds can
        // support JSON imports.  Allegedly this was fixed in
        // Microsoft/TypeScript#26825 but that doesn't seem to be working here,
        // so we will ignore complaints about this compiler setting.
        ...service
          .getCompilerOptionsDiagnostics()
          .filter((diagnostic): boolean => diagnostic.code !== 5070),
        ...service.getSyntacticDiagnostics(fileName),
        ...service.getSemanticDiagnostics(fileName)
      ];
      if (diagnostics.length > 0) {
        this._logDiagnostics(diagnostics);
      }

      assert(
        !output.emitSkipped,
        "The emit was skipped for an unknown reason."
      );

      assert(
        output.outputFiles.length === 2,
        `Expected 2 files to be emitted, got ${output.outputFiles.length}.`
      );

      const [sourceMapFile, outputFile] = output.outputFiles;
      assert(
        sourceMapFile.name.endsWith(".map"),
        "Expected first emitted file to be a source map"
      );
      assert(
        outputFile.name.endsWith(".js"),
        "Expected second emitted file to be JavaScript"
      );
      outputCode = moduleMetaData.outputCode = `${
        outputFile.text
      }\n//# sourceURL=${fileName}`;
      sourceMap = moduleMetaData.sourceMap = sourceMapFile.text;
    }

    moduleMetaData.scriptVersion = "1";
    return { outputCode, sourceMap };
  }
=======
>>>>>>> upstream/master

  /** Take a configuration string, parse it, and use it to merge with the
   * compiler's configuration options.  The method returns an array of compiler
   * options which were ignored, or `undefined`.
   */
  configure(path: string, configurationText: string): string[] | undefined {
    util.log("compile.configure", path);
    const { config, error } = ts.parseConfigFileTextToJson(
      path,
      configurationText
    );
    if (error) {
      this._logDiagnostics([error]);
    }
    const { options, errors } = ts.convertCompilerOptionsFromJson(
      config.compilerOptions,
      cwd()
    );
    if (errors.length) {
      this._logDiagnostics(errors);
    }
    const ignoredOptions: string[] = [];
    for (const key of Object.keys(options)) {
      if (
        ignoredCompilerOptions.includes(key) &&
        (!(key in this._options) || options[key] !== this._options[key])
      ) {
        ignoredOptions.push(key);
        delete options[key];
      }
    }
    Object.assign(this._options, options);
    return ignoredOptions.length ? ignoredOptions : undefined;
  }

  getCompilationSettings(): ts.CompilerOptions {
    util.log("getCompilationSettings()");
    return this._options;
  }

  /** Log TypeScript diagnostics to the console and exit */
  _logDiagnostics(diagnostics: ReadonlyArray<ts.Diagnostic>): never {
    const errMsg = os.noColor
      ? ts.formatDiagnostics(diagnostics, this)
      : ts.formatDiagnosticsWithColorAndContext(diagnostics, this);

    console.log(errMsg);
    // TODO The compiler isolate shouldn't call os.exit(). (In fact, it
    // shouldn't even have access to call that op.) Errors should be forwarded
    // to to the caller and the caller exit.
    return os.exit(1);
  }

  fileExists(_fileName: string): boolean {
    return notImplemented();
  }

<<<<<<< HEAD
  getScriptKind(fileName: ModuleFileName): ts.ScriptKind {
    this._log("getScriptKind()", fileName);
    const moduleMetaData = this._getModuleMetaData(fileName);
    if (moduleMetaData) {
      switch (moduleMetaData.mediaType) {
        case msg.MediaType.TypeScript:
          return ts.ScriptKind.TS;
        case msg.MediaType.JavaScript:
          return ts.ScriptKind.JS;
        case msg.MediaType.Json:
          return ts.ScriptKind.JSON;
        case msg.MediaType.Dylib:
          return ts.ScriptKind.TS;
        default:
          return this._options.allowJs ? ts.ScriptKind.JS : ts.ScriptKind.TS;
      }
    } else {
      return this._options.allowJs ? ts.ScriptKind.JS : ts.ScriptKind.TS;
=======
  readFile(_fileName: string): string | undefined {
    return notImplemented();
  }

  getSourceFile(
    fileName: string,
    languageVersion: ts.ScriptTarget,
    onError?: (message: string) => void,
    shouldCreateNewSourceFile?: boolean
  ): ts.SourceFile | undefined {
    assert(!shouldCreateNewSourceFile);
    util.log("getSourceFile", fileName);
    const moduleMetaData = this._resolveModule(fileName, ".");
    if (!moduleMetaData || !moduleMetaData.sourceCode) {
      return undefined;
>>>>>>> upstream/master
    }
    return ts.createSourceFile(
      fileName,
      moduleMetaData.sourceCode,
      languageVersion
    );
  }

  getDefaultLibFileName(_options: ts.CompilerOptions): string {
    return ASSETS + "/lib.deno_runtime.d.ts";
  }

  writeFile(
    fileName: string,
    data: string,
    writeByteOrderMark: boolean,
    onError?: (message: string) => void,
    sourceFiles?: ReadonlyArray<ts.SourceFile>
  ): void {
    util.log("writeFile", fileName);
    assert(sourceFiles != null && sourceFiles.length == 1);
    const sourceFileName = sourceFiles![0].fileName;

    if (fileName.endsWith(".map")) {
      // Source Map
      cache(".map", sourceFileName, data);
    } else if (fileName.endsWith(".js") || fileName.endsWith(".json")) {
      // Compiled JavaScript
      cache(".js", sourceFileName, data);
    } else {
      assert(false, "Trying to cache unhandled file type " + fileName);
    }
  }

  getCurrentDirectory(): string {
    return "";
  }

  getCanonicalFileName(fileName: string): string {
    // console.log("getCanonicalFileName", fileName);
    return fileName;
  }

  useCaseSensitiveFileNames(): boolean {
    return true;
  }

  getNewLine(): string {
    return "\n";
  }

  resolveModuleNames(
    moduleNames: string[],
    containingFile: string
  ): Array<ts.ResolvedModuleFull | undefined> {
    util.log("resolveModuleNames()", { moduleNames, containingFile });
    return moduleNames.map(
      (moduleName): ts.ResolvedModuleFull | undefined => {
        const moduleMetaData = this._resolveModule(moduleName, containingFile);
        if (moduleMetaData.moduleName) {
          const resolvedFileName = moduleMetaData.moduleName;
          // This flags to the compiler to not go looking to transpile functional
          // code, anything that is in `/$asset$/` is just library code
          const isExternalLibraryImport = moduleName.startsWith(ASSETS);
          const r = {
            resolvedFileName,
            isExternalLibraryImport,
            extension: getExtension(resolvedFileName, moduleMetaData.mediaType)
          };
          return r;
        } else {
          return undefined;
        }
      }
    );
  }

  private _resolveModule(specifier: string, referrer: string): ModuleMetaData {
    // Handle built-in assets specially.
    if (specifier.startsWith(ASSETS)) {
      const moduleName = specifier.split("/").pop()!;
      const assetName = moduleName.includes(".")
        ? moduleName
        : `${moduleName}.d.ts`;
      assert(assetName in assetSourceCode, `No such asset "${assetName}"`);
      const sourceCode = assetSourceCode[assetName];
      return {
        moduleName,
        filename: specifier,
        mediaType: msg.MediaType.TypeScript,
        sourceCode
      };
    }
    return fetchModuleMetaData(specifier, referrer);
  }
}

// provide the "main" function that will be called by the privileged side when
// lazy instantiating the compiler web worker
window.compilerMain = function compilerMain(): void {
  // workerMain should have already been called since a compiler is a worker.
  window.onmessage = ({ data }: { data: CompilerReq }): void => {
    const { rootNames, configPath, config } = data;
    const host = new Host();
    if (config && config.length) {
      const ignoredOptions = host.configure(configPath!, config);
      if (ignoredOptions) {
        console.warn(
          yellow(`Unsupported compiler options in "${configPath}"\n`) +
            cyan(`  The following options were ignored:\n`) +
            `    ${ignoredOptions
              .map((value): string => bold(value))
              .join(", ")}`
        );
      }
    }

    const options = host.getCompilationSettings();
    const program = ts.createProgram(rootNames, options, host);
    const emitResult = program!.emit();

    // TODO(ry) Print diagnostics in Rust.
    // https://github.com/denoland/deno/pull/2310

    const diagnostics = ts
      .getPreEmitDiagnostics(program)
      .concat(emitResult.diagnostics)
      .filter(
        ({ code }): boolean => {
          if (code === 2649) return false;
          // TS2691: An import path cannot end with a '.ts' extension. Consider
          // importing 'bad-module' instead.
          if (code === 2691) return false;
          // TS5009: Cannot find the common subdirectory path for the input files.
          if (code === 5009) return false;
          // TS5055: Cannot write file
          // 'http://localhost:4545/tests/subdir/mt_application_x_javascript.j4.js'
          // because it would overwrite input file.
          if (code === 5055) return false;
          // TypeScript is overly opinionated that only CommonJS modules kinds can
          // support JSON imports.  Allegedly this was fixed in
          // Microsoft/TypeScript#26825 but that doesn't seem to be working here,
          // so we will ignore complaints about this compiler setting.
          if (code === 5070) return false;
          return true;
        }
      );

    if (diagnostics.length > 0) {
      host._logDiagnostics(diagnostics);
      // The above _logDiagnostics calls os.exit(). The return is here just for
      // clarity.
      return;
    }

    postMessage(emitResult);

    // The compiler isolate exits after a single messsage.
    workerClose();
  };
};
