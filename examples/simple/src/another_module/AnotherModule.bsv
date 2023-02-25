interface AnotherModule;
    method Bool isAnotherModuleHookedUp;
endinterface

module mkAnotherModule(AnotherModule);
    EmbeddedModule embeddedModule = mkEmbeddedModule;

    method Bool isAnotherModuleHookedUp;
        return embeddedModule.isEmbeddedModuleHookedUp;
    endmethod
endmodule
