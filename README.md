# Hey Dot

Hey Dot is a fully local, voice-first desktop AI assistant.

It stays on top of your desktop, listens for "Hey Dot", captures your screen, runs local vision-language inference, and speaks back a concise answer. No cloud APIs, no internet dependency, and no data leaving your machine.

## Why Hey Dot

- 100% local AI runtime
- Wake word + voice query flow
- On-device screen understanding
- Spoken answers with low latency
- No API keys required

## How It Works

1. Hey Dot listens in the background for a wake phrase.
2. On wake, it starts a short active listening window.
3. It captures the current screen locally.
4. It sends the image + prompt to an Ollama model.
5. It speaks the response using macOS text-to-speech.

## Tech Stack

- Python 3
- customtkinter (desktop overlay UI)
- SpeechRecognition + PyAudio (microphone capture)
- faster-whisper (offline speech-to-text)
- mss + Pillow (screen capture)
- Ollama Python client (local model inference)
- numpy, python-dotenv
- macOS say command (voice output)

## Repository Structure

- `hey_dot.py`: Main desktop assistant runtime
- `requirements.txt`: Python dependencies
- `frontend/`: React + Vite showcase site for the project

## Prerequisites

- macOS
- Python 3.10+
- Ollama installed and running
- Microphone permission enabled for Terminal/Python
- Screen recording permission enabled for Terminal/Python

## Setup

```bash
python3 -m venv venv
source venv/bin/activate
pip install -r requirements.txt
```

Pull the default local vision-language model:

```bash
ollama pull moondream
```

## Run Hey Dot

```bash
source venv/bin/activate
python hey_dot.py
```

Runtime flow:

1. Wait for "Dot is Sleeping"
2. Say "Hey Dot"
3. Ask your question
4. Hear the spoken answer

## Optional Environment Variables

- `OLLAMA_MODEL`: Local Ollama model name (default: `moondream`)
- `WHISPER_MODEL`: Whisper checkpoint (default: `base.en`)

## Frontend

The frontend communicates the same positioning as the app: fully local, private, and cloud-free.

```bash
cd frontend
npm install
npm run dev
```

## Notes

- Wake-word detection includes phrase, token, and fuzzy matching for robustness.
- The current capture path targets `mss().monitors[1]`.
- Responses are intentionally short for natural voice playback.

## Current Limitations

- Wake word detection is STT-driven, not a dedicated keyword spotting model.
- Performance depends on hardware and local model choice.
- Multi-monitor selection is not yet configurable.
