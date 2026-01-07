export default function AuraVisionPage() {
  return (
    <div className="space-y-4">
      <h1 className="text-3xl font-semibold tracking-tight">Aura Vision</h1>
      <p className="max-w-2xl text-sm leading-6 text-zinc-700 dark:text-zinc-300">
        An ONNX Runtime demo that validates the “AI-native” promise: tensor shapes are part of the
        type, ONNX model IO shapes are extracted, and Z3 enforces shape equality for inference.
      </p>
      <div className="rounded-2xl border border-black/10 bg-white/60 p-6 dark:border-white/10 dark:bg-black/40">
        <div className="text-sm font-medium">Highlights</div>
        <ul className="mt-3 list-disc pl-5 text-sm text-zinc-700 dark:text-zinc-300">
          <li>ai.load_model reads the ONNX contract</li>
          <li>model.infer(x) is verifier-enforced shape-safe</li>
          <li>Inference callsites are tagged for AI optimization</li>
        </ul>
      </div>
    </div>
  );
}
