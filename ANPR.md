# ANPR and vehicle events

`GET /api/events/stream` sends named `vehicle.passed` SSE events when an eligible vehicle track expires. Eligibility follows the existing zones and their virtual-line rules. SSE is available whenever the REST API is enabled outside report mode and has no replay. Events can also be published to Redis independently of the REST API. Disabling ANPR leaves vehicle events enabled, with `plate: null` and no image.

```shell
curl -N http://localhost:42001/api/events/stream
```

Plate detection and OCR are optional. Configure both models under `[anpr.plates]` and `[anpr.ocr]` using [data/conf.toml](data/conf.toml), then enable the cascade:

```toml
[anpr]
    enable = true
    image = "vehicle"
```

Recognition makes at most three attempts per track and stops after two identical nonempty readings. The final `plate.ocr` contains the fused `number`, `mean_confidence`, `has_conflicts`, `reference_attempt` and `positions` with observations and competing alternatives. Missing plate or OCR results are `null`; agreement is not a guarantee of correctness. Only `anpr.enable` and `anpr.image` are available through `GET/PATCH /api/config`; model settings remain in TOML. Save API changes with `GET /api/mutations/save_toml`, then restart to apply.

`anpr.image` is optional and accepts:

| Value | `frame_base64` contents |
| --- | --- |
| `""` | `null`, no image encoding |
| `"full"` | Full processing frame, without overlays |
| `"vehicle"` | Crop matching `vehicle.bbox` |
| `"plate"` | Crop matching the original `plate.bbox`, without OCR padding; `null` if no plate was found |

Images require `anpr.enable = true` and are JPEG, quality 90, encoded as standard Base64 without a data-URL prefix. The event's `frame_type` identifies the configured image mode, or `""` when image export is disabled. For browser display, prepend `data:image/jpeg;base64,` to a non-null `frame_base64`. Encoding failures are logged and produce `null` without preventing event delivery.

The image and all boxes belong to the selected reference observation. If no attempt finds a plate, `full` and `vehicle` use the last attempt with an encoded image and its vehicle metadata. `frame_width` and `frame_height` always describe the full processing frame, including when a crop is sent. Crop dimensions are the corresponding bbox's `width` and `height`. Coordinates are pixels: vehicle relative to frame, plate relative to vehicle, OCR positions relative to the original plate. OCR uses padding internally, but symbol boxes are clipped to the original plate before export; a position absent from the reference attempt has a `null` bbox.

Only the selected image mode is retained. Attempt images are stored as JPEG and Base64 is generated at event completion. Full-frame mode also compresses frames when pending recognition candidates change; tracks selecting the same frame share the JPEG. This bounds retained images per track but adds more encoding work than vehicle or plate mode.

## Redis publication

Enable publication of the same final event JSON through Redis Pub/Sub:

```toml
[redis_publisher.vehicle_events]
    enable = true
    channel_name = "VEHICLE_EVENTS"
```

This section is optional and defaults to disabled, with channel `VEHICLE_EVENTS`. Its switch is independent of `redis_publisher.enable`, which controls statistics. By default, events use the parent Redis connection settings and share the actual connection with statistics when both are enabled. Events also work when statistics or the REST API are disabled. Report mode publishes neither statistics nor vehicle events.

To send events to a different Redis, add an optional connection block:

```toml
[redis_publisher.vehicle_events.connection]
    host = "another-redis"
    port = 6379
    username = ""
    password = ""
    db_index = 0
```

In this block, `host` is required obviously. And omitted values use port 6379, no credentials and database 0 as is.

Parent credentials are not inherited in that case. So you could just delete the entire block to use the shared parent connection. These settings are also available through `GET/PATCH /api/config`; `connection: null` restores the shared connection. Save and restart to apply.

Subscribe before a track ends, using the configured Redis host, credentials and channel:

```shell
redis-cli -h localhost -p 6379 SUBSCRIBE VEHICLE_EVENTS
```

The payload is the JSON object below, including the same `event_id`, OCR and optional image as SSE, without SSE framing. Serialization and Redis I/O run in a background worker with a persistent connection and a bounded queue of 64 pending messages per connection. A full queue drops new messages; publication failures are logged and discarded, with reconnection attempted for the next message. There is no replay or durable storage, and pending messages may be lost at shutdown. Redis Pub/Sub does not retain messages for disconnected subscribers.

## Event JSON

The SSE event name is `vehicle.passed`; its `data` field contains the JSON object below. This is a complete illustrative payload with image export disabled. Only the last symbol has a conflict: attempt 1 read `7`, attempt 2 read `1`.

```json
{
  "event_id": "52f7d9d7-b21a-46f7-a9bf-27e820080811",
  "type": "vehicle.passed",
  "equipment_id": "demo-camera",
  "track_id": "41f342f3-9b43-4961-802b-53709708cdc6",
  "started_at": "2026-10-09T09:00:00Z",
  "ended_at": "2026-10-09T09:00:03Z",
  "vehicle": {
    "class": "car",
    "confidence": 0.95,
    "bbox": {"x": 100, "y": 120, "width": 300, "height": 280}
  },
  "plate": {
    "class": "civilian",
    "confidence": 0.92,
    "bbox": {"x": 90, "y": 220, "width": 110, "height": 30},
    "ocr": {
      "number": "A123BC71",
      "mean_confidence": 0.9,
      "has_conflicts": true,
      "reference_attempt": 2,
      "positions": [
        {
          "position": 0,
          "row": 0,
          "class": "A",
          "mean_confidence": 0.9,
          "status": "agreement",
          "bbox": {"x": 5, "y": 7, "width": 9, "height": 16},
          "observations": [{"attempt": 1, "confidence": 0.88}, {"attempt": 2, "confidence": 0.92}],
          "alternatives": []
        },
        {
          "position": 1,
          "row": 0,
          "class": "1",
          "mean_confidence": 0.9,
          "status": "agreement",
          "bbox": {"x": 17, "y": 7, "width": 9, "height": 16},
          "observations": [{"attempt": 1, "confidence": 0.88}, {"attempt": 2, "confidence": 0.92}],
          "alternatives": []
        },
        {
          "position": 2,
          "row": 0,
          "class": "2",
          "mean_confidence": 0.9,
          "status": "agreement",
          "bbox": {"x": 29, "y": 7, "width": 9, "height": 16},
          "observations": [{"attempt": 1, "confidence": 0.88}, {"attempt": 2, "confidence": 0.92}],
          "alternatives": []
        },
        {
          "position": 3,
          "row": 0,
          "class": "3",
          "mean_confidence": 0.9,
          "status": "agreement",
          "bbox": {"x": 41, "y": 7, "width": 9, "height": 16},
          "observations": [{"attempt": 1, "confidence": 0.88}, {"attempt": 2, "confidence": 0.92}],
          "alternatives": []
        },
        {
          "position": 4,
          "row": 0,
          "class": "B",
          "mean_confidence": 0.9,
          "status": "agreement",
          "bbox": {"x": 53, "y": 7, "width": 9, "height": 16},
          "observations": [{"attempt": 1, "confidence": 0.88}, {"attempt": 2, "confidence": 0.92}],
          "alternatives": []
        },
        {
          "position": 5,
          "row": 0,
          "class": "C",
          "mean_confidence": 0.9,
          "status": "agreement",
          "bbox": {"x": 65, "y": 7, "width": 9, "height": 16},
          "observations": [{"attempt": 1, "confidence": 0.88}, {"attempt": 2, "confidence": 0.92}],
          "alternatives": []
        },
        {
          "position": 6,
          "row": 0,
          "class": "7",
          "mean_confidence": 0.9,
          "status": "agreement",
          "bbox": {"x": 82, "y": 7, "width": 9, "height": 16},
          "observations": [{"attempt": 1, "confidence": 0.88}, {"attempt": 2, "confidence": 0.92}],
          "alternatives": []
        },
        {
          "position": 7,
          "row": 0,
          "class": "1",
          "mean_confidence": 0.9,
          "status": "conflict",
          "bbox": {"x": 94, "y": 7, "width": 9, "height": 16},
          "observations": [{"attempt": 2, "confidence": 0.9}],
          "alternatives": [{"class": "7", "mean_confidence": 0.8, "observations": [{"attempt": 1, "confidence": 0.8}]}]
        }
      ]
    }
  },
  "frame_base64": null,
  "frame_type": "",
  "frame_width": 960,
  "frame_height": 540
}
```

| Field | Meaning |
| --- | --- |
| `event_id` | Unique ID of this final event. |
| `type` | `vehicle.passed`, also used as the SSE event name. |
| `equipment_id` | Installation ID from the configuration. |
| `track_id` | Vehicle track ID. |
| `started_at`, `ended_at` | UTC timestamps of the first and last observed detections of the track. Publication happens later, when the track expires. These are not the image capture time or virtual-line crossing time. |
| `vehicle` | Vehicle class, detector confidence and bbox from the reference observation. Class names follow the configured vehicle model. |
| `plate` | Plate class, detector confidence, original bbox without padding, and OCR result. `null` when no plate was found or ANPR is disabled. Plate classes follow `anpr.plates.net_classes`. |
| `plate.ocr` | Fused OCR result, or `null` if no usable reading is available. |
| `frame_base64` | Standard Base64 of JPEG bytes without a data-URL prefix, or `null` if disabled, unavailable or encoding failed. |
| `frame_type` | Effective image mode: `""`, `"full"`, `"vehicle"` or `"plate"`. A requested image can still be `null`, for example when no plate is found in plate mode. |
| `frame_width`, `frame_height` | Dimensions of the full processing frame, even when the transmitted image is a crop. |

| OCR field | Meaning |
| --- | --- |
| `number` | Selected classes concatenated in the reading order of `positions`. |
| `mean_confidence` | Arithmetic mean of the selected positions' `mean_confidence` values. Each position has equal weight, regardless of how many attempts support it. This is not a calibrated probability that the whole number is correct. |
| `has_conflicts` | `true` if any position has competing classes. `false` does not guarantee a correct reading. |
| `reference_attempt` | One-based attempt number supplying the vehicle/plate geometry, image and available symbol boxes. |
| `positions` | Symbols aligned across OCR attempts, in reading order. |

| Position field | Meaning |
| --- | --- |
| `position` | Zero-based index across the whole number; it does not reset on a new row. |
| `row` | Zero-based text row, useful for plates with multiple rows. |
| `class` | Selected OCR class from `anpr.ocr.net_classes`. |
| `mean_confidence` | Arithmetic mean of `confidence` across the observations supporting this selected class. |
| `status` | `single`: one observation; `agreement`: multiple observations with the same class; `conflict`: competing classes at this position. |
| `bbox` | Real symbol box at this position in the reference attempt, relative to the original plate. It may come from a detection whose class differs from the fused choice. `null` if the position is absent from the reference attempt; no box is invented from another image. |
| `observations` | Support for the selected class. Each entry has the one-based `attempt` and its original detector `confidence`. |
| `alternatives` | Competing classes only, usually an empty array. Each alternative has `class`, its own `mean_confidence`, and supporting `observations`. |

All bboxes have integer `x`, `y`, `width` and `height` in pixels. The origin is the parent's top-left corner; X increases rightward and Y downward. Vehicle boxes are relative to the full frame, plate boxes to the vehicle, and symbol boxes to the original plate. In the example, the first symbol starts at frame coordinates `x = 100 + 90 + 5 = 195`, `y = 120 + 220 + 7 = 347`. Changing the image mode does not change this coordinate system. A vehicle image has `vehicle.bbox.width` by `vehicle.bbox.height` pixels; a plate image has `plate.bbox.width` by `plate.bbox.height` pixels.
