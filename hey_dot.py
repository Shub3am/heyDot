import io
import os
import difflib
import re
import sys
import threading
import time
import numpy as np
import speech_recognition as sr
import mss
import ollama
from faster_whisper import WhisperModel
from PIL import Image
from dotenv import load_dotenv
import customtkinter as ctk
import subprocess

# Load environment variables
load_dotenv()

# qwen3-vl:4b — reliable VL model with fast no-think mode (~1-2s on Apple Silicon).
# Pull it with: ollama pull qwen3-vl:4b
MODEL = os.environ.get("OLLAMA_MODEL", "qwen3-vl:4b")

# Local Whisper model for fully offline STT (no internet required).
# Uses Apple CPU with int8 quantisation — fast on Apple Silicon.
_whisper_model_name = os.environ.get("WHISPER_MODEL", "base.en")
print(f"Loading Whisper model '{_whisper_model_name}'...")
WHISPER = WhisperModel(_whisper_model_name, device="cpu", compute_type="int8")
print("Whisper model ready.")
class HelioOverlay:
    def __init__(self):
        ctk.set_appearance_mode("dark")
        ctk.set_default_color_theme("blue")
        
        self.root = ctk.CTk()
        self.root.title("Dot")
        
        self.root.attributes("-topmost", True)
        self.root.attributes("-alpha", 0.90)
        
        self.root.geometry("380x110+50+50") 
        
        self.frame = ctk.CTkFrame(self.root, corner_radius=20, fg_color="#1c1c1e", border_width=1, border_color="#333333")
        self.frame.pack(expand=True, fill="both", padx=15, pady=15)
        
        # Left side: Indicator Canvas
        self.indicator_canvas = ctk.CTkCanvas(self.frame, width=30, height=30, bg="#1c1c1e", highlightthickness=0)
        self.indicator_canvas.pack(side="left", padx=(20, 15))
        self.indicator_id = self.indicator_canvas.create_oval(5, 5, 25, 25, fill="#8e8e93", outline="")
        
        # Right side text container
        self.text_frame = ctk.CTkFrame(self.frame, fg_color="transparent")
        self.text_frame.pack(side="left", fill="both", expand=True, padx=(0, 20), pady=15)
        
        self.label = ctk.CTkLabel(self.text_frame, text="Booting...", text_color="#ffffff", font=ctk.CTkFont(family="Helvetica Neue", size=18, weight="bold"), anchor="w")
        self.label.pack(fill="x")
        
        self.sub_label = ctk.CTkLabel(self.text_frame, text="Initializing audio engine", text_color="#8e8e93", font=ctk.CTkFont(family="Helvetica Neue", size=13), anchor="w")
        self.sub_label.pack(fill="x")

        self.expecting_query = False
        self.is_processing_query = False
        self.listening_until = 0.0
        self._prefetched_screen = None
        self.screen_context_mode = "on"  # VL-only: always capture screen
        
        self.recognizer = sr.Recognizer()
        # Tune recognizer for faster and more sensitive wake-word pickup.
        self.recognizer.dynamic_energy_threshold = True
        self.recognizer.energy_threshold = 180
        self.recognizer.pause_threshold = 0.6
        self.recognizer.phrase_threshold = 0.2
        self.recognizer.non_speaking_duration = 0.2
        
        self.animate_indicator()

        # We start the audio initialization in a background thread 
        # because sr.Microphone() can block the UI from showing up on macOS macOS!
        init_thread = threading.Thread(target=self.init_audio, daemon=True)
        init_thread.start()

    def animate_indicator(self):
        if not hasattr(self, 'pulse_dir'):
            self.pulse_dir = 1
            self.pulse_radius = 10
            
        if self.expecting_query or self.is_processing_query:
            self.pulse_radius += 0.5 * self.pulse_dir
            if self.pulse_radius >= 12:
                self.pulse_dir = -1
            elif self.pulse_radius <= 7:
                self.pulse_dir = 1
                
            center = 15
            self.indicator_canvas.coords(
                self.indicator_id, 
                center - self.pulse_radius, center - self.pulse_radius, 
                center + self.pulse_radius, center + self.pulse_radius
            )
        else:
            if self.pulse_radius != 10:
                self.pulse_radius = 10
                self.indicator_canvas.coords(self.indicator_id, 5, 5, 25, 25)
                
        self.root.after(40, self.animate_indicator)

    def update_ui(self, title, subtext="", color="#ffffff"):
        print(f"[UI State] {title} - {subtext}")
        def _update():
            self.label.configure(text=title, text_color=color)
            self.sub_label.configure(text=subtext)
            self.indicator_canvas.itemconfig(self.indicator_id, fill=color)
        self.root.after(0, _update)

    def speak(self, text):
        """Synchronous TTS inside inference thread for reliability."""
        self.update_ui("Dot is Speaking", "Relaying information...", "#0a84ff")
        try:
            subprocess.run(["say", text])
        except Exception as e:
            print("TTS error", e)

    def capture_screen(self):
        """Returns a compressed JPEG bytes object ready for Ollama VL."""
        try:
            with mss.mss() as sct:
                monitor = sct.monitors[1]
                screenshot = sct.grab(monitor)
                img = Image.frombytes("RGB", screenshot.size, screenshot.rgb)

            # Aggressively downscale — VL models don't need full resolution.
            try:
                resample = Image.Resampling.LANCZOS
            except AttributeError:
                resample = Image.LANCZOS  # type: ignore[attr-defined]
            img.thumbnail((480, 270), resample)

            buf = io.BytesIO()
            img.save(buf, format="JPEG", quality=55, optimize=True)
            data = buf.getvalue()
            print(f"[Screen] {len(data) // 1024}KB")
            return data
        except Exception as e:
            print("Screen capture failed", e)
            return None

    def process_query(self, query):
        self.update_ui("Dot is Thinking", "Analyzing your request...", "#ffd60a")
        
        def run_inference():
            try:
                started_at = time.time()

                # Always capture screen for visual context.
                image_bytes = self._prefetched_screen
                self._prefetched_screen = None
                if image_bytes is None:
                    self.update_ui("Capturing Screen", "Extracting visual context...", "#ffd60a")
                    image_bytes = self.capture_screen()
                if image_bytes is None:
                    image_bytes = self.capture_screen()  # one more try

                system_msg = {
                    "role": "system",
                    "content": (
                        "You are Dot, a concise desktop assistant that can see the user's screen in real time. "
                        "You directly observe what is on the screen — never say 'screenshot', 'image', or 'attached'. "
                        "Speak as if you are naturally looking at their screen alongside them. "
                        "Reply in 1-2 short spoken sentences. Do NOT use markdown, bullet points, or any formatting."
                    ),
                }
                user_msg = {
                    "role": "user",
                    "content": query,
                    "images": [image_bytes] if image_bytes else [],
                }

                # Assistant prefill with empty think block disables chain-of-thought → fast response.
                self.update_ui("Dot is Thinking", "Generating response...", "#ffd60a")
                response = ollama.chat(
                    model=MODEL,
                    messages=[
                        system_msg,
                        user_msg,
                        {"role": "assistant", "content": "<think>\n\n</think>\n\n"},
                    ],
                    keep_alive="30m",
                    options={
                        "temperature": 0.2,
                        "num_predict": 120,
                    },
                )

                answer = (response.message.content or "").strip()

                if not answer:
                    answer = "I could not generate a response right now. Please try again."

                print(f"Dot says: {answer}")
                self.speak(answer)

                print(f"[Latency] total={time.time() - started_at:.2f}s")
            except Exception as e:
                print(f"Error processing: {e}")
                self.speak("Sorry, I encountered an error.")
            finally:
                self.is_processing_query = False
                self.expecting_query = False
                self.update_ui("Dot is Sleeping", "Say 'Hey Dot' to wake me up", "#8e8e93")

        # Run inference in background so we don't block the audio callback thread
        threading.Thread(target=run_inference, daemon=True).start()

    def trigger_active_listening(self):
        os.system('afplay /System/Library/Sounds/Glass.aiff &')
        self.update_ui("Listening...", "Awaiting your command", "#30d158")
        self.expecting_query = True
        self.listening_until = time.time() + 6.0
        # Pre-capture screen now so it's ready the moment the query arrives.
        threading.Thread(target=self._prefetch_screen, daemon=True).start()

    def _prefetch_screen(self):
        self._prefetched_screen = self.capture_screen()

    def is_wake_word(self, text):
        normalized = re.sub(r"[^a-z0-9\s]", " ", text.lower())
        normalized = re.sub(r"\s+", " ", normalized).strip()

        # Phrase-level check first — two-word phrases are far more reliable.
        wake_phrases = {"hey dot", "hey, dot", "a dot", "hey doc", "hey dog", "hey dot."}
        if any(p in normalized for p in wake_phrases):
            return True

        # Exact token match for standalone short variants.
        wake_tokens = {"dot", "dott", "dotty", "dat", "tot", "daut", "doc", "computer", "buddy"}
        tokens = normalized.split()
        if any(t in wake_tokens for t in tokens):
            return True

        # Fuzzy fallback for misrecognitions.
        return any(difflib.SequenceMatcher(None, token, "dot").ratio() >= 0.75 for token in tokens)

    def audio_callback(self, recognizer, audio):
        if self.is_processing_query:
            return # Ignore audio while Gemini is processing

        try:
            # Transcribe locally using Whisper — no internet needed.
            raw = audio.get_raw_data(convert_rate=16000, convert_width=2)
            audio_array = np.frombuffer(raw, dtype=np.int16).astype(np.float32) / 32768.0
            segments, _ = WHISPER.transcribe(
                audio_array,
                language="en",
                beam_size=1,
                vad_filter=True,
                vad_parameters={"min_silence_duration_ms": 300},
            )
            text = " ".join(seg.text for seg in segments).strip().lower()
            print(f"[Heard]: {text}")
            if not text:
                raise sr.UnknownValueError()
            
            if self.expecting_query:
                # The user spoke their command after the wake word!
                if self.is_wake_word(text) and len(text.split()) <= 2:
                    # Ignore repeated wake words and keep waiting for the real command.
                    self.listening_until = time.time() + 4.0
                    self.update_ui("Listening...", "Awaiting your command", "#30d158")
                    return

                print(">>> Treating as user query!")
                self.expecting_query = False
                self.is_processing_query = True
                self.process_query(text)
            else:
                # Looking for wake word
                if self.is_wake_word(text):
                    print(">>> Wake word detected!")
                    self.trigger_active_listening()
                    
        except sr.UnknownValueError:
            # We captured noise, but no speech
            if self.expecting_query:
                # Keep listening until grace period expires.
                if time.time() >= self.listening_until:
                    self.expecting_query = False
                    self.update_ui("Dot is Sleeping", "Say 'Hey Dot' to wake me up", "#8e8e93")
                else:
                    self.update_ui("Listening...", "Awaiting your command", "#30d158")
        except Exception as e:
            print(f"Callback error: {e}")
            if self.expecting_query:
                if time.time() >= self.listening_until:
                    self.expecting_query = False
                    self.update_ui("Dot is Sleeping", "Say 'Hey Dot' to wake me up", "#8e8e93")

    def init_audio(self):
        try:
            self.microphone = sr.Microphone()
            with self.microphone as source:
                print("Adjusting ambient noise...")
                self.recognizer.adjust_for_ambient_noise(source, duration=1)
                # Bias slightly lower than measured ambient threshold for sensitivity.
                self.recognizer.energy_threshold = max(120, self.recognizer.energy_threshold * 0.7)
                
            # start listening in background using the library's threaded listener
            self.stop_listening = self.recognizer.listen_in_background(self.microphone, self.audio_callback, phrase_time_limit=3)
            
            self.update_ui("Dot is Sleeping", "Say 'Hey Dot' to wake me up", "#8e8e93")
            print("Background audio engine complete!")
        except Exception as e:
            print(f"Failed to init microphone: {e}")
            self.update_ui("Audio Error", "Check your microphone", "#ff3b30")

    def run(self):
        self.root.lift()
        # Bring python window to front
        os.system("open -a Python")
        self.root.mainloop()

if __name__ == "__main__":
    app = HelioOverlay()
    app.run()
