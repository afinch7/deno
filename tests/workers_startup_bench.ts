const workerCount = 200;

async function bench(): Promise<void> {
  const workers: Worker[] = [];
  Array(workerCount).forEach(async () => {
    const worker = new Worker("tests/subdir/bench_worker.ts");
    const promise = new Promise(resolve => {
      worker.onmessage = e => {
        if (e.data.cmdId === 0) resolve();
      };
    });
    worker.postMessage({ cmdId: 0, action: 2 });
    await promise;
    workers.push(worker);
  });
  console.log("Done creating workers closing workers!");
  for (const worker of workers) {
    worker.postMessage({ action: 3 });
    await worker.closed; // Required to avoid a cmdId not in table error.
  }
  console.log("Finished!");
}

bench();
